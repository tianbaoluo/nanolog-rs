use std::{io, ptr};
use std::sync::Arc;
use crate::log::{rdtsc, Level, LogFn};
use crate::{tscns, StagingBuffer};
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
        hdr.func = func as u64;

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
  let queue = Arc::new(StagingBuffer::new());
  {
    let queue = queue.clone();
    std::thread::spawn(move || {
      let res = core_affinity::set_for_current( core_affinity::CoreId { id: 7 });
      if let Err(e) = run(queue) {
        println!("Run log-backend error: {:?}", e);
      }
    });
  }
  LoggerHandle {
    queue,
  }
}

fn run(queue: Arc<StagingBuffer>) -> io::Result<()> {
  let consumer = Consumer {
    q: queue.as_ref(),
  };

  loop {
    while let Some((hdr, payload, total)) = consumer.front() {
      // unsafe {
      //   // decode using (*hdr).msg_type / userdata and payload bytes...
      //   let _ = (hdr, payload, total);
      // }
      consumer.pop();
    }
    std::hint::spin_loop();
  }

  Ok(())
}
