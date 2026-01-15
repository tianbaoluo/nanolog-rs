use std::fmt;
use std::fmt::Display;
use std::time::Duration;
use bytemuck::{Pod, Zeroable};
use hft_log_demo::args2::UserPod;
use hft_log_demo::hft_info;
use hft_log_demo::run_log::init_logger;

fn main() {
  let logger = init_logger(1024);

  let timer = minstant::Instant::now();
  for id in 0..10_000 {
    let user = UserData {
      x: id,
      a: id,
      y: id as u64,
    };
    hft_info!(logger, "curr {} u {}", id, user);
  }
  let time_cost = timer.elapsed();
  eprintln!("cost-us={}", time_cost.as_micros());

  // println!("wait 2sec");
  std::thread::sleep(Duration::from_millis(5000));
  // println!("Done");
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct UserData {
  x: u32,
  a: u32,
  y: u64,
}

impl Display for UserData {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "user {{ x={}, a={}, y={} }}", self.x, self.a, self.y)
  }
}

impl UserPod for UserData {}