use std::cell::{Cell, UnsafeCell};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::io;
use std::io::Write;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crossbeam_channel::{Receiver, Sender};
use crate::log::{rdtsc, LogEntry};
use crate::{spsc_queue, StagingBuffer};

struct RegMsg {
  cons: spsc_queue::Consumer<LogEntry>,
  tid: u32,
}

pub struct LoggerHandle {
  prod: spsc_queue::Producer<LogEntry>,
  reg_tx: Sender<RegMsg>,
  capacity: usize,
}

// impl Clone for LoggerHandle {
//   fn clone(&self) -> Self {
//     Self {
//       prod: self.prod,
//       reg_tx: self.reg_tx.clone(),
//       capacity: self.capacity,
//     }
//   }
// }

struct TlsProd {
  inited: Cell<bool>,
  prod: UnsafeCell<MaybeUninit<spsc_queue::Producer<LogEntry>>>,
  tid: Cell<u32>,
}

unsafe impl Sync for TlsProd {}

static NEXT_TID: AtomicU32 = AtomicU32::new(1);

impl TlsProd {
  // #[inline(always)]
  // fn get_mut(&self, logger: &LoggerHandle) -> &mut spsc_queue::Producer<LogEntry> {
  //   if !self.inited.get() {
  //     let (prod, cons) = spsc_queue::spsc_queue::<LogEntry>(logger.capacity);
  //     let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed); //get_tid();
  //     self.tid.set(tid);
  //
  //     let _ = logger.reg_tx.send(RegMsg { cons, tid });
  //
  //     unsafe { (*self.prod.get()).write(prod) };
  //     self.inited.set(true);
  //   }
  //   unsafe { (*self.prod.get()).assume_init_mut() }
  // }

  // #[inline(always)]
  // fn init(&self, logger: &mut LoggerHandle) {
  //   if !self.inited.get() {
  //     let (prod, cons) = spsc_queue::spsc_queue::<LogEntry>(logger.capacity);
  //     let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed); //get_tid();
  //     self.tid.set(tid);
  //
  //     let _ = logger.reg_tx.send(RegMsg { cons, tid });
  //
  //     unsafe { (*self.prod.get()).write(prod) };
  //     logger.prod = self.prod.get().as_mut_ptr();
  //     self.inited.set(true);
  //   }
  //   // unsafe { (*self.prod.get()).assume_init_mut() }
  // }
}

// thread_local! {
//     static TLS_PROD: TlsProd = TlsProd {
//         inited: Cell::new(false),
//         prod: UnsafeCell::new(MaybeUninit::uninit()),
//         tid: Cell::new(0),
//     };
// }

impl LoggerHandle {
  #[inline(always)]
  pub fn push(&mut self, e: LogEntry) {
    let mut log_entry = e;
    while let Err(e) = self.prod.push(log_entry) {
      log_entry = e;
    }
    // let _ = self.prod.push(e); // 满了就丢；你可以加 dropped 计数
    // TLS_PROD.with(|tls| {
    //   let p = tls.get_mut(self);
    //   let _ = p.push(e); // 满了就丢；你可以加 dropped 计数
    // });
  }

  #[inline(always)]
  pub fn push_write<F: FnOnce(&mut LogEntry)>(&mut self, f: F) -> bool {
    self.prod.push_write(f).is_ok() // 满了就丢；你可以加 dropped 计数
  }
}

#[inline(always)]
fn level_str(l: u64) -> &'static str {
  match l {
    0 => "trace",
    1 => "debug",
    2 => "\x1b[32minfo\x1b[m ",
    3 => "\x1b[31mwarn\x1b[m ",
    4 => "\x1b[31merror\x1b[m",
    _ => "unk  ",
  }
}

// =============================
// Logger thread: collect consumers + K-way heap merge by tsc
// =============================
struct QState {
  cons: spsc_queue::Consumer<LogEntry>,
  head: Option<LogEntry>,
  tid: u32,
}


struct LoggerThread {
  reg_rx: Receiver<RegMsg>,
  qs: Vec<QState>,
  heap: BinaryHeap<Reverse<(u64, usize)>>, // (tsc, qid)
  empty: Vec<usize>,
  empty_cursor: usize,
  clock: TscClock,
  prefix: PrefixCache,
}

impl LoggerThread {
  fn new(reg_rx: Receiver<RegMsg>) -> Self {
    Self {
      reg_rx,
      qs: Vec::new(),
      heap: BinaryHeap::new(),
      empty: Vec::new(),
      empty_cursor: 0,
      clock: TscClock::calibrate(),
      prefix: PrefixCache::new(),
    }
  }

  // fn add_consumer(&mut self, msg: RegMsg) {
  //   let mut cons = msg.cons;
  //   let mut st = QState {
  //     cons,
  //     head: None,
  //     tid: msg.tid,
  //   };
  //   self.qs.push(st);
  //   // let qid = self.qs.len();
  //   // if let Some(e) = st.cons.pop() {
  //   //   let t = e.tsc;
  //   //   st.head = Some(e);
  //   //   self.qs.push(st);
  //   //   self.heap.push(Reverse((t, qid)));
  //   // } else {
  //   //   self.qs.push(st);
  //   //   self.empty.push(qid);
  //   // }
  // }

  // #[inline(always)]
  // fn refill_head(&mut self, qid: usize) {
  //   if self.qs[qid].head.is_none() {
  //     if let Some(e) = self.qs[qid].cons.pop() {
  //       let t = e.tsc;
  //       self.qs[qid].head = Some(e);
  //       self.heap.push(Reverse((t, qid)));
  //     } else {
  //       self.empty.push(qid);
  //     }
  //   }
  // }
  //
  // #[inline(always)]
  // fn scan_empty_budget(&mut self, budget: usize) {
  //   let mut b = 0;
  //   while b < budget && !self.empty.is_empty() {
  //     let len = self.empty.len();
  //     let idx = self.empty_cursor % len;
  //     let qid = self.empty[idx];
  //
  //     if self.qs[qid].head.is_none() {
  //       if let Some(e) = self.qs[qid].cons.pop() {
  //         let t = e.tsc;
  //         self.qs[qid].head = Some(e);
  //         self.heap.push(Reverse((t, qid)));
  //         self.empty.swap_remove(idx);
  //       } else {
  //         self.empty_cursor = self.empty_cursor.wrapping_add(1);
  //       }
  //     } else {
  //       self.empty.swap_remove(idx);
  //     }
  //
  //     b += 1;
  //   }
  // }

  // #[inline(always)]
  // fn write_header(&mut self, out: &mut dyn Write, e: &LogEntry) -> io::Result<()> {
  //   // tsc -> epoch_ns
  //   let epoch_ns = self.clock.tsc_to_epoch_ns(e.tsc);
  //   let sec = epoch_ns / 1_000_000_000;
  //   let sub = (epoch_ns % 1_000_000_000) as u32;
  //   let ms = sub / 1_000_000;
  //   let us = (sub / 1_000) % 1000;
  //
  //   // per-second prefix cache: "MM-DD HH:MM:SS"
  //   if self.prefix.sec != sec {
  //     self.prefix.refresh(sec);
  //   }
  //
  //   // [MM-DD HH:MM:SS.mmm.uuu level site tid]
  //   out.write_all(b"[")?;
  //   out.write_all(&self.prefix.buf[..self.prefix.len])?;
  //   out.write_all(b".")?;
  //   let mut tmp = [0u8; 3];
  //   three_digits(&mut tmp, ms);
  //   out.write_all(&tmp)?;
  //   out.write_all(b".")?;
  //   three_digits(&mut tmp, us);
  //   out.write_all(&tmp)?;
  //   out.write_all(b" ")?;
  //   out.write_all(level_str(e.level).as_bytes())?;
  //   out.write_all(b" ")?;
  //   Ok(())
  // }

  fn run(mut self) -> io::Result<()> {
    let mut qs = Vec::with_capacity(64);
    loop {
      while let Ok(msg) = self.reg_rx.try_recv() {
        // self.add_consumer(msg);
        let mut cons = msg.cons;
        let mut st = QState {
          cons,
          head: None,
          tid: msg.tid,
        };
        qs.push(st);
      }

      // let mut out = io::stdout();
      for qs in qs.iter_mut() {
        let tid = qs.tid;
        while let Some(log_entry) = qs.cons.pop() {
          // self.write_header(&mut out, &log_entry)?;
          // // let len = e.len as usize;
          // (log_entry.func)(&mut out, tid, &log_entry.data)?;
          // out.write_all(b"\n")?;
        }
      }
      // out.flush()?;
      // drop(out);
      // drop(stdout);

      // println!("park");
      std::thread::park_timeout(Duration::from_micros(100));
      // println!("unpark");
      // break;
    }
    println!("Done");

    // let stdout = io::stdout();
    // let mut out = io::BufWriter::new(stdout.lock());
    //
    // loop {
    //   while let Ok(msg) = self.reg_rx.try_recv() {
    //     self.add_consumer(msg);
    //   }
    //
    //   // 小预算扫描空队列（兜底）
    //   self.scan_empty_budget(4);
    //
    //   if let Some(Reverse((_t, qid))) = self.heap.pop() {
    //     let e = self.qs[qid].head.take().unwrap();
    //     let tid = self.qs[qid].tid;
    //
    //     self.write_header(&mut out, &e)?;
    //     // let len = e.len as usize;
    //     (e.func)(&mut out, tid, &e.data);
    //     out.write_all(b"\n")?;
    //
    //     self.refill_head(qid);
    //   } else {
    //     std::thread::yield_now();
    //   }
    // }
  }
}

// =============================
// init_logger
// =============================
pub fn init_logger(capacity: usize) -> LoggerHandle {
  let (reg_tx, reg_rx) = crossbeam_channel::unbounded();

  std::thread::spawn(move || {
    let res = core_affinity::set_for_current( core_affinity::CoreId { id: 7 });
    let lt = LoggerThread::new(reg_rx);
    if let Err(e) = lt.run() {
      println!("Run log-backend error: {:?}", e);
    }
  });

  // let queue = Arc::new(StagingBuffer::new());
  let (prod, cons) = spsc_queue::spsc_queue::<LogEntry>(capacity);
  let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed); //get_tid();
  let _ = reg_tx.send(RegMsg { cons, tid });

  LoggerHandle { prod, reg_tx, capacity }
}

// =============================
// TSC -> epoch_ns mapping + prefix cache (优化#1)
// =============================
pub struct TscClock {
  base_tsc: u64,
  base_epoch_ns: u64,
  hz: f64,
}

impl TscClock {
  pub fn calibrate() -> Self {
    // base
    let base_epoch_ns = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_nanos() as u64;

    // hz
    let t0 = rdtsc();
    let s0 = std::time::Instant::now();
    std::thread::sleep(Duration::from_millis(10));
    let t1 = rdtsc();
    let dt_ns = s0.elapsed().as_nanos() as f64;
    let hz = (t1 - t0) as f64 * 1e9 / dt_ns;

    Self {
      base_tsc: t1,
      base_epoch_ns,
      hz,
    }
  }

  #[inline(always)]
  pub fn tsc_to_epoch_ns(&self, tsc: u64) -> u64 {
    let d_cycles = tsc.wrapping_sub(self.base_tsc) as f64;
    let d_ns = d_cycles * 1e9 / self.hz;
    self.base_epoch_ns.wrapping_add(d_ns.max(0.0) as u64)
  }
}

struct PrefixCache {
  sec: u64,
  buf: [u8; 32], // "MM-DD HH:MM:SS" = 14 bytes
  len: usize,
}

impl PrefixCache {
  fn new() -> Self {
    Self {
      sec: u64::MAX,
      buf: [0u8; 32],
      len: 0,
    }
  }

  #[inline(never)]
  fn refresh(&mut self, sec: u64) {
    self.sec = sec;

    // localtime_r
    #[cfg(unix)]
    unsafe {
      let mut t: libc::tm = std::mem::zeroed();
      let mut tt: libc::time_t = sec as libc::time_t;
      libc::localtime_r(&tt as *const libc::time_t, &mut t as *mut libc::tm);

      let mon = (t.tm_mon + 1) as u32;
      let mday = t.tm_mday as u32;
      let hour = t.tm_hour as u32;
      let min = t.tm_min as u32;
      let ssec = t.tm_sec as u32;

      // "MM-DD HH:MM:SS"
      let b = &mut self.buf;
      two_digits(&mut b[0..2], mon);
      b[2] = b'-';
      two_digits(&mut b[3..5], mday);
      b[5] = b' ';
      two_digits(&mut b[6..8], hour);
      b[8] = b':';
      two_digits(&mut b[9..11], min);
      b[11] = b':';
      two_digits(&mut b[12..14], ssec);

      self.len = 14;
    }

    #[cfg(not(unix))]
    {
      // fallback: just show sec
      self.len = 0;
    }
  }
}

#[inline(always)]
fn two_digits(dst: &mut [u8], x: u32) {
  dst[0] = b'0' + ((x / 10) as u8);
  dst[1] = b'0' + ((x % 10) as u8);
}
#[inline(always)]
fn three_digits(dst: &mut [u8], x: u32) {
  dst[0] = b'0' + ((x / 100) as u8);
  dst[1] = b'0' + (((x / 10) % 10) as u8);
  dst[2] = b'0' + ((x % 10) as u8);
}
