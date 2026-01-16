use std::{io, ptr};
use std::ptr::slice_from_raw_parts;
use std::sync::Arc;
use std::time::Duration;
use crate::log::{rdtsc, Level, LogFn};
use crate::{tscns, StagingBuffer};
use crate::console_sink::ConsoleBatchSink;
use crate::spsc_var_queue_opt::{Consumer, Producer};

pub struct LoggerHandle {
  pub queue: Arc<StagingBuffer>,
}

impl LoggerHandle {
  pub fn publish_args<A: Copy>(&self, level: Level, func: LogFn, args: &A) -> bool {
    let prod = Producer {
      q: self.queue.as_ref(),
    };

    let len = size_of::<A>();
    if let Some((hdr, payload, payload_cap, total, _blk_sz)) = prod.try_alloc(len) {
      unsafe {
        let hdr = &mut (*hdr);
        hdr.level = level as u8 as u32;
        hdr.tsc = tscns::read_tsc();
        hdr.log_func = func as u64;

        ptr::copy_nonoverlapping(args as *const A as *const u8, payload, len);
        prod.commit(hdr, total);
      }
      true
    } else {
      false
    }
  }
}

pub fn init_logger(capacity: usize) -> LoggerHandle {
  tscns::init(tscns::INIT_CALIBRATE_NANOS, tscns::CALIBRATE_INTERVAL_NANOS);

  std::thread::spawn(move || {
    loop {
      tscns::calibrate();
      // println!("calibrate");
      std::thread::sleep(Duration::from_nanos(tscns::CALIBRATE_INTERVAL_NANOS as u64));
    }
  });

  let queue = Arc::new(StagingBuffer::new());
  {
    let queue = queue.clone();
    std::thread::spawn(move || {
      let res = core_affinity::set_for_current( core_affinity::CoreId { id: 7 });
      if let Err(e) = run(1, queue) {
        println!("Run log-backend error: {:?}", e);
      }
    });
  }
  LoggerHandle {
    queue,
  }
}

fn run(tid: usize, queue: Arc<StagingBuffer>) -> io::Result<()> {
  let consumer = Consumer {
    q: queue.as_ref(),
  };
  let mut console_sink = ConsoleBatchSink::new();

  let mut no_data = 0;
  let mut num_loop = 0usize;
  loop {
    no_data = 1;
    while let Some((hdr, payload, total)) = consumer.front() {
      unsafe {
        let log_header = &*hdr;
        let log_payload = &*slice_from_raw_parts(payload, total as usize);
        console_sink.on_record(tid, log_header, log_payload).unwrap();
      }
      consumer.pop();
      no_data = 0;
    }
    num_loop += no_data;

    if num_loop >= 1024 {
      console_sink.on_idle(tscns::read_tsc()).unwrap();
    }
    std::hint::spin_loop();
  }

  Ok(())
}
