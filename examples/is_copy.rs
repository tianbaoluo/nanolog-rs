use bytemuck::{Pod, Zeroable};
use hft_log_demo::args2::args2;

trait NotCopy {
  const IS_COPY: bool = false;
}
impl <T> NotCopy for T {}

// Concrete wrapper type where `IS_COPY` becomes `true` if `T: Copy`.
struct IsCopy<T>(std::marker::PhantomData<T>);

impl<T: Copy> IsCopy<T> {
  // Because this is implemented directly on `IsCopy`, it has priority over
  // the `NotCopy` trait impl.
  //
  // Note: this is a *totally different* associated constant from that in
  // `NotCopy`. This does not specialize the `NotCopy` trait impl on `IsCopy`.
  const IS_COPY: bool = true;
}

struct PrimitiveTag<T>(std::marker::PhantomData<T>);
impl PrimitiveTag<u32> {
  const ARG_TAG: u8 = 8;
}

impl PrimitiveTag<u64> {
  const ARG_TAG: u8 = 64;
}

trait PodTag {
  const ARG_TAG: u8 = 13;
}

impl <T> PodTag for T {}

fn main() {
  let args = args2(123u32, 345u64);
  let len = size_of_val(&args);
  println!("size-of(u32,u64)={} #u64={}", len, len >> 3);
  let a = IsCopy::<u32>::IS_COPY;
  let b = IsCopy::<Vec<u32>>::IS_COPY;
  println!("a = {} b = {}", a, b);

  let u32_tag = PrimitiveTag::<u32>::ARG_TAG;
  let u64_tag = PrimitiveTag::<u64>::ARG_TAG;
  let f64_tag = PrimitiveTag::<f64>::ARG_TAG;
  let user_tag = PrimitiveTag::<UserData>::ARG_TAG;
  println!("u32 = {} u64 = {} f64 = {} user = {}", u32_tag, u64_tag, f64_tag, user_tag);
  print_tag::<u32>("u32");
}

trait Arg {
  const ARG_TAG: u8;
}

impl Arg for u32 {
  const ARG_TAG: u8 = 0;
}

fn print_tag<T>(name: &'static str) {
  println!("{} = {}", name, PrimitiveTag::<T>::ARG_TAG);
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct UserData {
  x: u32,
  a: u32,
  y: u64,
}
