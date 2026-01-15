use std::fmt;
use std::fmt::{Display, Formatter};
use std::intrinsics::transmute;
use bytemuck::{Pod, Zeroable};

pub trait Arg: Display + Copy + Clone {
  const ARG_TAG: u8;
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(transparent)]
pub struct ArgF64(f64);

impl Display for ArgF64 {
  #[inline]
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f64::fmt(&self.0, f)
  }
}

impl Arg for ArgF64 {
  const ARG_TAG: u8 = 0;
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(transparent)]
pub struct ArgU64(u64);

impl Display for ArgU64 {
  #[inline]
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    u64::fmt(&self.0, f)
  }
}
impl Arg for ArgU64 {
  const ARG_TAG: u8 = 1;
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(transparent)]
pub struct ArgI64(i64);

impl Display for ArgI64 {
  #[inline]
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    i64::fmt(&self.0, f)
  }
}
impl Arg for ArgI64 {
  const ARG_TAG: u8 = 2;
}

pub trait IntoArg {
  type D: Arg;
  fn into_arg(self) -> Self::D;
}

impl IntoArg for u32 {
  type D = ArgU64;

  #[inline(always)]
  fn into_arg(self) -> Self::D {
    ArgU64(self as _)
  }
}

impl IntoArg for u64 {
  type D = ArgU64;

  #[inline(always)]
  fn into_arg(self) -> Self::D {
    ArgU64(self)
  }
}

#[inline(always)]
pub(crate) fn repr_as<T>(slice: &[u8]) -> &T {
  unsafe {
    &*(slice.as_ptr() as *const T)
  }
}

#[inline(always)]
pub(crate) fn repr_off_as<T>(slice: &[u8], offset: usize) -> &T {
  unsafe {
    &*(slice.as_ptr().add(offset) as *const T)
  }
}

pub trait UserPod: Display + Copy + Pod + Zeroable {
  fn decode(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let d = repr_as::<Self>(bytes);
    d.fmt(f)
  }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct UserPodSnap<T: UserPod> {
  decode_fn: u64,
  data: T,
}

unsafe impl <T: UserPod> Zeroable for UserPodSnap<T> {}
unsafe impl <T: UserPod> Pod for UserPodSnap<T> {}

impl <T: UserPod> Display for UserPodSnap<T> {
  #[inline]
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    T::fmt(&self.data, f)
  }
}

impl <T: UserPod> Arg for UserPodSnap<T> {
  const ARG_TAG: u8 = size_of::<T>() as u8 + 8;
}

impl <T: UserPod> IntoArg for T {
  type D = UserPodSnap<T>;

  fn into_arg(self) -> Self::D {
    UserPodSnap {
      decode_fn: T::decode as u64,
      data: self,
    }
  }
}

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C, packed)]
pub struct Args2<T1: Arg, T2: Arg> {
  pub tag1: u8,
  pub tag2: u8,
  _pad: [u8; 6],
  pub arg1: T1,
  pub arg2: T2,
}

#[inline]
pub fn args2<T1: IntoArg, T2: IntoArg>(arg1: T1, arg2: T2) -> Args2::<T1::D, T2::D> {
  let arg1 = arg1.into_arg();
  let arg2 = arg2.into_arg();
  Args2 {
    tag1: T1::D::ARG_TAG,
    tag2: T2::D::ARG_TAG,
    _pad: [0; 6],
    arg1,
    arg2,
  }
}

pub fn decode_fmt_args2(bytes: &[u8]) {
  let mut offset = 8;
  let tag1 = bytes[0];
  let tag2 = bytes[1];
  match tag1 {
    0 => {
      let v = repr_off_as::<f64>(bytes, offset);
      offset += 8;
      println!("f64 {}", v);
    },
    1 => {
      let v = repr_off_as::<u64>(bytes, offset);
      offset += 8;
      println!("u64 {}", v);
    },
    2 => {
      let v = repr_off_as::<i64>(bytes, offset);
      offset += 8;
      println!("i64 {}", v);
    },
    len => {
      let decode_fn = *repr_off_as::<u64>(bytes, offset);
      let start = offset + 8;
      offset += len as usize;
      let snap_bytes = SnapBytes {
        decode_fn,
        bytes: &bytes[start..offset],
      };
      println!("user-pod: {}", snap_bytes);
    },
  }
  match tag2 {
    0 => {
      let v = repr_off_as::<f64>(bytes, offset);
      offset += 8;
      println!("f64 {}", v);
    },
    1 => {
      let v = repr_off_as::<u64>(bytes, offset);
      offset += 8;
      println!("u64 {}", v);
    },
    2 => {
      let v = repr_off_as::<i64>(bytes, offset);
      offset += 8;
      println!("i64 {}", v);
    },
    len => {
      let decode_fn = *repr_off_as::<u64>(bytes, offset);
      let start = offset + 8;
      offset += len as usize;
      let snap_bytes = SnapBytes {
        decode_fn,
        bytes: &bytes[start..offset],
      };
      println!("user-pod: {}", snap_bytes);
    },
  }
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