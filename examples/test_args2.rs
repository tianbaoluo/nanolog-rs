use std::fmt;
use std::fmt::Display;
use std::hint::black_box;
use bytemuck::{Pod, Zeroable};
use hft_log_demo::args2::{args2, decode_fmt_args2, UserPod};
// use hft_log_demo::args::Args2;

fn main() {
  let u = UserData {
    x: 434,
    a: 46413,
    y: 234827913,
  };

  let args = args2(8764_u32, u);
  println!("size-of: {} = tag=8 + arg1={} + arg2.decode-fn={} + arg2.data={}", size_of_val(&args), size_of::<u64>(), size_of::<u64>(), size_of::<UserData>());
  println!("t1={} t2={}", args.tag1, args.tag2);

  println!("decode");
  let bytes = bytemuck::bytes_of(&args);
  println!("data: {:?}", bytes);
  decode_fmt_args2(bytes);

  let timer = minstant::Instant::now();
  for id in 0..1_000_000u32 {
    let u = UserData {
      x: id,
      a: id,
      y: id as u64,
    };
    let args = args2(id, u);
    black_box(args);
  }
  let time_cost = timer.elapsed();
  println!("cost-us={}", time_cost.as_micros());
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