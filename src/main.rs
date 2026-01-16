use std::time::Duration;
use hft_log_demo::hft_info;
use hft_log_demo::log::rdtsc;
use hft_log_demo::run_log2::init_logger;

const ROUND: usize = 256;
const NUM_LOG: usize = 128;

fn main() {
  let res = core_affinity::set_for_current( core_affinity::CoreId { id: 6 });

  let logger = init_logger(1024 * 128);

  // std::thread::sleep(Duration::from_millis(500));
  let num_log = NUM_LOG as u32;
  let mut total_cost_cycles = 0;
  let mut batch_costs = Vec::<u64>::with_capacity(ROUND);
  let mut num_droped = 0usize;

  for _ in 0..ROUND {
    // let timer = std::time::Instant::now();
    let start_cycles = tsc_start();
    for id in 0..num_log {
      let id = std::hint::black_box(id);
      let ok = hft_info!(logger, "curr {} u {}", id, id);
      num_droped += !ok as usize;
      // std::thread::sleep(Duration::from_millis(5000_000));
    }
    let end_cycles = tsc_end();
    let cost_cycles = end_cycles - start_cycles;
    // let cost_ns = timer.elapsed().as_nanos();
    total_cost_cycles += cost_cycles;
    batch_costs.push(cost_cycles);
    std::hint::spin_loop();
    std::thread::park_timeout(Duration::from_micros(10_000));
  }

  //
  println!("wait 5sec");
  std::thread::sleep(Duration::from_millis(5_000));

  println!("num-droped={}", num_droped);

  let total_logs = (ROUND as u128) * (NUM_LOG as u128);
  let avg_per_log = total_cost_cycles as f64 / total_logs as f64;
  println!("cost-ns={} avg = {}", total_cost_cycles, avg_per_log);
  let min = *batch_costs.iter().min().unwrap();
  let max = *batch_costs.iter().max().unwrap();
  let p50 = percentile_ns(batch_costs.clone(), 0.50);
  let p90 = percentile_ns(batch_costs.clone(), 0.90);
  let p99 = percentile_ns(batch_costs.clone(), 0.99);
  let p999 = percentile_ns(batch_costs.clone(), 0.999);
  println!("== burst bench ==");
  println!("ROUND={} NUM_LOG={} total_cost_cycles={}", ROUND, NUM_LOG, total_cost_cycles);
  println!("avg per log: {:.3} cycles", avg_per_log);
  println!(
    "batch cycles: min={} p50={} p90={} p99={} p999={} max={}",
    min, p50, p90, p99, p999, max
  );
  // println!("cost-cycles={} avg-cycles = {}", total_cost_cycles, avg_per_log);
  println!("Done");
}

fn percentile_ns(mut v: Vec<u64>, p: f64) -> u64 {
  v.sort_unstable();
  let n = v.len();
  // let idx = ((n as f64 - 1.0) * p).round() as usize;
  let idx = ((n as f64 - 1.0) * p).floor() as usize;
  v[idx.min(n - 1)]
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub fn tsc_start() -> u64 {
  unsafe { core::arch::x86_64::_mm_lfence(); }
  unsafe { core::arch::x86_64::_rdtsc() }
}

#[cfg(target_os = "linux")]
#[inline(always)]
pub fn tsc_end() -> u64 {
  let t = unsafe { core::arch::x86_64::_rdtsc() };
  unsafe { core::arch::x86_64::_mm_lfence(); }
  t
}

#[cfg(not(target_os = "linux"))]
#[inline(always)]
pub fn tsc_start() -> u64 {
  rdtsc()
}

#[cfg(not(target_os = "linux"))]
#[inline(always)]
pub fn tsc_end() -> u64 {
  rdtsc()
}
