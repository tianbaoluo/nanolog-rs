use std::fmt;
use std::fmt::Display;
use std::time::Duration;
use bytemuck::{Pod, Zeroable};
use hft_log_demo::args2::UserPod;
use hft_log_demo::hft_info;
use hft_log_demo::run_log2::init_logger;

// const ROUND: usize = 1_000_000_000;
const ROUND: usize = 10_000;

fn main() {
  let mut logger = init_logger(1024 * 16);

  let round = ROUND as u32;
  // let timer = minstant::Instant::now();
  let timer = std::time::Instant::now();
  for id in 0..round {
    // let user = UserData {
    //   x: id,
    //   a: id,
    //   y: id as u64,
    // };
    let ok = hft_info!(logger, "curr {} u {}", id, id);
  }
  let time_cost_ns = timer.elapsed().as_nanos();
  println!("cost-ns={} avg = {}", time_cost_ns, time_cost_ns as f64 / round as f64);
  //
  println!("wait 5sec");
  std::thread::sleep(Duration::from_millis(5000));
  println!("cost-ns={} avg = {}", time_cost_ns, time_cost_ns as f64 / round as f64);
  println!("Done");
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