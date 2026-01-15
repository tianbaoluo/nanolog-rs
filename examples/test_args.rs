use std::fmt;
use std::fmt::Display;
use bytemuck::{Pod, Zeroable};
use hft_log_demo::args::{Args2, UserPod};

fn main() {
  let u = UserData {
    x: 434,
    a: 46413,
    y: 234827913,
  };

  let args2 = Args2::new(8764_u32, u);
  println!("size-of: {} = 8 + {} + {}", size_of_val(&args2), size_of::<u32>(), size_of::<UserData>());
  println!("t1={} t2={}", args2.tag1, args2.tag2);
}

// fn use_args2<T1: Display + Copy + Pod, T2: Display + Copy + Pod>(arg1: T1, arg2: T2) {
//   let args = Args2::new(arg1, arg2);
//   fn __hft_shim(out: &mut dyn std::io::Write, p: *const u8) {
//     // let a = unsafe { &*(p as *const Args2<T1, T2>) };
//     // let arg1 = a.arg1;
//     // let arg2 = a.arg2;
//     // println!("a1 = {} a2 = {}", arg1, arg2);
//   }
// }

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