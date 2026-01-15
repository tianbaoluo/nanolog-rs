use std::fmt;
use std::fmt::{Display, Pointer};
use std::mem::transmute;
use bytemuck::{Pod, Zeroable};

fn main() {
  println!("size-of={}", size_of::<UserData>());
  let u = UserData {
    x: 434,
    a: 46413,
    y: 234827913,
  };
  println!("test-fn={}", UserData::test as u64);

  println!("snap-u64={}", Snap::<u64>::decode as u64);
  println!("snap-i32={}", Snap::<i32>::decode as u64);

  let snap = Snap::from(u);
  let bytes = bytemuck::bytes_of(&snap);
  println!("bytes: {:?}", bytes);

  let decode_fn = *repr_as::<u64>(bytes);
  println!("decode-fn={}", decode_fn);

  // let decode_fn = unsafe { transmute::<_, DecodeFn>(decode_fn) };
  // decode_fn(&bytes[8..]);

  let xx = SnapBytes {
    decode_fn,
    bytes: &bytes[8..],
  };
  println!("got xx: {}", xx);
}

type DecodeFn = fn(&[u8], &mut fmt::Formatter<'_>) -> fmt::Result;

struct SnapBytes<'a> {
  decode_fn: u64,
  bytes: &'a [u8],
}

impl <'a> Display for SnapBytes<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let decode_fn = unsafe { transmute::<_, DecodeFn>(self.decode_fn) };
    decode_fn(self.bytes, f)
  }
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C, packed)]
struct Snap<T: Display + Copy + Clone + Pod + Zeroable> {
  decode_fn: u64,
  data: T,
}

impl <T: Display + Copy + Clone + Pod + Zeroable> Snap<T> {
  fn from(data: T) -> Self {
    let decode_fn = Self::decode as u64;
    Snap {
      decode_fn,
      data,
    }
  }

  fn decode(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let d = repr_as::<T>(bytes);
    d.fmt(f)
  }
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

#[inline(always)]
pub(crate) fn repr_as<T>(slice: &[u8]) -> &T {
  unsafe {
    &*(slice.as_ptr() as *const T)
  }
}

impl UserData {
  fn test(&self) {

  }
}