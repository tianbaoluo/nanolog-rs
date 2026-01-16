use std::io::{self, Write};
use std::mem::transmute;
use crate::format::{lut_msus, TidCache, TimeCache, LEVEL_STRS};
use crate::log::LogFn;
use crate::my_bytes_mut::MyBytesMut;
use crate::spsc_var_queue_opt::MsgHeader;
use crate::tscns;



/// -------- Console batch sink --------
pub struct ConsoleBatchSink {
  // 批量 buffer
  batch: Vec<u8>,
  // 每条 log 拼接的 scratch（可选，用于减少 batch 里反复 extend）
  // 你也可以直接 batch.extend(prefix); batch.extend(payload)...
  // 这里保留一个 scratch 是为了你后续加 timestamp/level 时更顺手。
  scratch: MyBytesMut,

  // flush 策略
  flush_bytes: usize,
  flush_interval_cycles: i64,
  last_flush_cycles: i64,

  time_cache: TimeCache, // like 01-16 09:33:36 T00
  tid_cache: TidCache, // like T=00


  // stdout lock（只在 consumer 线程使用）
  // out: io::StdoutLock<'static>,
}

impl ConsoleBatchSink {
  pub fn new() -> Self {
    // 注意：StdoutLock 生命周期问题：最简单的做法是在 consumer 线程里构造 sink，
    // 并用 Box::leak 把 stdout 变成 'static（仅骨架用；生产里你可以把 lock 放到 run() 里）。
    // let stdout = Box::leak(Box::new(io::stdout()));
    // let out = stdout.lock();

    // let flush_interval_cycles = (500_000.0 / tscns::get_ns_per_tsc()) as i64;
    // let flush_interval_cycles = us_to_cycles(500, tsc_hz);

    Self {
      batch: Vec::with_capacity(256 * 1024),
      scratch: MyBytesMut::with_capacity(512),

      flush_bytes: 256 * 1024,
      flush_interval_cycles: 1_500_000,
      last_flush_cycles: tscns::read_tsc(),

      // prefix: TidCache::new(32),
      // out,
      time_cache: TimeCache::new(),
      tid_cache: TidCache::new(32),
    }
  }

  #[inline(always)]
  fn should_flush(&self, now_cycles: i64) -> bool {
    self.batch.len() >= self.flush_bytes || now_cycles.wrapping_sub(self.last_flush_cycles) >= self.flush_interval_cycles
  }

  #[inline(always)]
  fn flush_now(&mut self) -> io::Result<()> {
    println!("flush_now");
    if self.batch.is_empty() {
      self.last_flush_cycles = tscns::read_tsc();
      return Ok(());
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    out.write_all(&self.batch)?;
    // 如果你希望“500us 到就一定可见”，可以加 flush；
    // 但 flush 可能更贵。通常只在时间触发时 flush。
    out.flush()?;
    self.batch.clear();
    self.last_flush_cycles = tscns::read_tsc();
    Ok(())
  }

  /// 处理一条日志（payload 已经是 bytes；你也可以传入结构化参数）
  #[inline(always)]
  pub fn on_record(&mut self, tid: usize, log_meta: &MsgHeader, log_payload: &[u8]) -> io::Result<()> {
    // println!("on_record");
    let level = log_meta.level as usize;
    let tsc = log_meta.tsc;
    let log_fn = unsafe { transmute::<_, LogFn>(log_meta.log_func) };

    let curr_ns = tscns::tsc2ns(tsc);

    let curr_sec = curr_ns / 1_000_000_000;
    let sub_ns = curr_ns % 1_000_000_000;

    let sub_us = sub_ns / 1_000;        // 0..999_999
    let curr_ms = (sub_us / 1_000) as usize;   // 0..999
    let curr_us = (sub_us % 1_000) as usize;   // 0..999

    self.scratch.clear();
    self.scratch.push(b'[');
    self.time_cache.refresh_dt(curr_sec, self.scratch.unfilled());
    self.scratch.advance(TimeCache::TIME_LEN);
    lut_msus(self.scratch.unfilled(), curr_ms, curr_us);
    self.scratch.advance(8);
    self.scratch.push(b' ');

    self.tid_cache.write(tid, self.scratch.unfilled());
    self.scratch.advance(TidCache::TID_LEN);
    self.scratch.push(b' ');

    unsafe {
      self.scratch.extend_from_slice(LEVEL_STRS.get_unchecked(level).as_bytes());
    }

    (log_fn)(&mut self.scratch, log_payload)?;

    // self.scratch.extend_from_slice(payload);
    self.scratch.push(b'\n');

    self.batch.extend_from_slice(self.scratch.result());

    // 2) flush 条件
    if self.should_flush(tsc) {
      self.flush_now()?;
    }
    Ok(())
  }

  /// 在空闲时也调用一下：如果 500us 到了，强制 flush（即使没有新日志）
  #[inline(always)]
  pub fn on_idle(&mut self, now_cycles: i64) -> io::Result<()> {
    if !self.batch.is_empty()
      && now_cycles.wrapping_sub(self.last_flush_cycles) >= self.flush_interval_cycles
    {
      self.flush_now()?;
    }
    Ok(())
  }
}

// for test
// fn __hft_shim(out: &mut MyBytesMut, bytes: &[u8]) -> std::io::Result<()> {
//   let src_loc = crate::log::SourceLocation::__new(module_path!(), file!(), line!());
//   out.extend_from_slice(src_loc.module_path.as_bytes());
//   out.extend_from_slice(b"::");
//   out.extend_from_slice(src_loc.file_name().as_bytes());
//   out.extend_from_slice(src_loc.line.to_string().as_bytes());
//   out.extend_from_slice(b"] ");
//   let tag1 = bytes[0];
//   let tag2 = bytes[1];
//   let (arg1, offset) = crate::args2::decode(tag1, bytes, 8);
//   let (arg2, _) = crate::args2::decode(tag2, bytes, offset);
//
//   write!(out, "x={} y={}", arg1, arg2)
// }

/// -------- consumer loop 骨架 --------
/// 你把这里的 `try_pop_record()` 替换成你自己的队列读取即可。
pub fn console_consumer_loop(mut sink: ConsoleBatchSink) -> io::Result<()> {
  loop {
    let now = tscns::read_tsc();

    // 伪代码：从每个 producer 的队列里拉数据
    // 你可能是：for pid in 0..N { while let Some(rec)=q[pid].front() { ... } }
    let mut progressed = false;

    // ----- 这里替换成你的 drain 逻辑 -----
    // if let Some((pid, payload_bytes)) = try_pop_record() {
    //     sink.on_record(pid, payload_bytes, now)?;
    //     progressed = true;
    // }
    // -----------------------------------

    if !progressed {
      // 没数据：也要按 500us 强制 flush
      sink.on_idle(now)?;
      // 这里不要立刻 park 太久；console 线程可以短暂 spin 再 park
      std::hint::spin_loop();
    }
  }
}