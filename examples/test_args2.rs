use std::fmt;
use std::fmt::Display;
use bytemuck::{Pod, Zeroable};
use hft_log_demo::args2::{args2, decode_fmt_args2, UserPod};
// use hft_log_demo::args::Args2;

fn main() {
  let u = UserData {
    x: 434,
    a: 46413,
    y: 234827913,
  };

  let args2 = args2(8764_u32, u);
  println!("size-of: {} = tag=8 + arg1={} + arg2.decode-fn={} + arg2.data={}", size_of_val(&args2), size_of::<u64>(), size_of::<u64>(), size_of::<UserData>());
  println!("t1={} t2={}", args2.tag1, args2.tag2);

  println!("decode");
  let bytes = bytemuck::bytes_of(&args2);
  println!("data: {:?}", bytes);
  decode_fmt_args2(bytes);
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