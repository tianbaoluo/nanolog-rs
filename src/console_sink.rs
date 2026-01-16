use std::io::{self, Write};
use crate::tscns;

/// -------- Prefix cache --------
/// 按 pid / tid 缓存固定前缀（比如 "[pid=2] " / "[T3] " / 你的颜色码等）
struct TidCache {
  prefix: Vec<Vec<u8>>,
}

impl TidCache {
  fn new(capacity: usize) -> Self {
    let mut prefix = Vec::with_capacity(capacity);
    for tid in 0..capacity {
      // 这里随便举例：你可以换成更复杂的前缀
      let mut p = Vec::with_capacity(4);
      write!(&mut p, "[T{}]", tid).unwrap();
      // p.extend_from_slice(b"[tid=");
      // push_u32(&mut p, tid as u32);
      // p.extend_from_slice(b"] ");
      prefix.push(p);
    }
    Self { prefix }
  }

  #[inline(always)]
  fn get(&self, tid: usize) -> &[u8] {
    // pid 由你保证合法
    &self.prefix[tid]
  }
}

/// 无分配整数拼接（极简版；你也可以换 itoa crate）
#[inline(always)]
fn push_u32(dst: &mut Vec<u8>, mut x: u32) {
  // 写到栈上再 reverse
  let mut buf = [0u8; 10];
  let mut i = 0;
  if x == 0 {
    dst.push(b'0');
    return;
  }
  while x > 0 {
    let d = (x % 10) as u8;
    buf[i] = b'0' + d;
    i += 1;
    x /= 10;
  }
  while i > 0 {
    i -= 1;
    dst.push(buf[i]);
  }
}

/// -------- Console batch sink --------
pub struct ConsoleBatchSink {
  // 批量 buffer
  batch: Vec<u8>,
  // 每条 log 拼接的 scratch（可选，用于减少 batch 里反复 extend）
  // 你也可以直接 batch.extend(prefix); batch.extend(payload)...
  // 这里保留一个 scratch 是为了你后续加 timestamp/level 时更顺手。
  scratch: Vec<u8>,

  // flush 策略
  flush_bytes: usize,
  flush_interval_cycles: i64,
  last_flush_cycles: i64,

  // prefix cache
  prefix: TidCache,

  // stdout lock（只在 consumer 线程使用）
  out: io::StdoutLock<'static>,
}

impl ConsoleBatchSink {
  pub fn new() -> Self {
    // 注意：StdoutLock 生命周期问题：最简单的做法是在 consumer 线程里构造 sink，
    // 并用 Box::leak 把 stdout 变成 'static（仅骨架用；生产里你可以把 lock 放到 run() 里）。
    let stdout = Box::leak(Box::new(io::stdout()));
    let out = stdout.lock();

    let flush_interval_cycles = (500_000.0 / tscns::get_ns_per_tsc()) as i64;
    // let flush_interval_cycles = us_to_cycles(500, tsc_hz);

    Self {
      batch: Vec::with_capacity(256 * 1024),
      scratch: Vec::with_capacity(512),

      flush_bytes: 256 * 1024,
      flush_interval_cycles,
      last_flush_cycles: tscns::read_tsc(),

      prefix: TidCache::new(32),
      out,
    }
  }

  #[inline(always)]
  fn should_flush(&self, now_cycles: i64) -> bool {
    self.batch.len() >= self.flush_bytes || now_cycles.wrapping_sub(self.last_flush_cycles) >= self.flush_interval_cycles
  }

  #[inline(always)]
  fn flush_now(&mut self) -> io::Result<()> {
    if self.batch.is_empty() {
      self.last_flush_cycles = tscns::read_tsc();
      return Ok(());
    }
    self.out.write_all(&self.batch)?;
    // 如果你希望“500us 到就一定可见”，可以加 flush；
    // 但 flush 可能更贵。通常只在时间触发时 flush。
    self.out.flush()?;
    self.batch.clear();
    self.last_flush_cycles = tscns::read_tsc();
    Ok(())
  }

  /// 处理一条日志（payload 已经是 bytes；你也可以传入结构化参数）
  #[inline(always)]
  pub fn on_record(&mut self, tid: usize, payload: &[u8], now_cycles: i64) -> io::Result<()> {
    // 1) 拼接：prefix + payload + '\n'
    let pref = self.prefix.get(tid);

    // 这里用 scratch 只是为了以后扩展更方便
    self.scratch.clear();
    self.scratch.extend_from_slice(pref);
    self.scratch.extend_from_slice(payload);
    self.scratch.push(b'\n');

    self.batch.extend_from_slice(&self.scratch);

    // 2) flush 条件
    if self.should_flush(now_cycles) {
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