use std::time::Duration;
use hft_log_demo::tscns;

fn main() {
  tscns::init(tscns::INIT_CALIBRATE_NANOS, tscns::CALIBRATE_INTERVAL_NANOS);

  std::thread::spawn(move || {
    loop {
      tscns::calibrate();
      println!("calibrate");
      std::thread::sleep(Duration::from_nanos(tscns::CALIBRATE_INTERVAL_NANOS as u64));
    }
  });

  println!("cpu ns-per-tick = {}", tscns::get_ns_per_tsc());

  for i in 0..100 {
    let now_ns = now_ns();
    let curr_ns = tscns::read_nanos();
    // let curr_tsc = tscns::read_tsc();
    println!("#{}: {} = {} - {}", i, curr_ns as i128 - now_ns, curr_ns, now_ns);

    std::thread::park_timeout(Duration::from_millis(100));
  }
}

#[inline(always)]
pub fn now_ns() -> i128 {
  unsafe {
    let mut time: libc::timespec = std::mem::zeroed();
    if libc::clock_gettime(libc::CLOCK_REALTIME, &mut time) == -1 {
      unreachable!("Call libc::clock_gettime(libc::CLOCK_REALTIME, &mut time) error: {:?}", std::io::Error::last_os_error());
    }
    time.tv_sec as i128 * 1000_000_000 + time.tv_nsec as i128
  }
}
