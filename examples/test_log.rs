use std::fmt;
use std::fmt::Display;
use std::time::Duration;
use bytemuck::{Pod, Zeroable};
use hft_log_demo::args2::UserPod;
use hft_log_demo::hft_info;
use hft_log_demo::run_log::init_logger;

fn main() {
  let logger = init_logger(1024);

  for id in 10..100 {
    let user = UserData {
      x: id,
      a: id + 2,
      y: id as u64 * 100,
    };
    hft_info!(logger, "curr {} u {}", id, user);
  }

  println!("wait 2sec");
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