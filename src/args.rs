use std::fmt::Display;
use std::num::Wrapping;
use bytemuck::{Pod, Zeroable};

// const TAG_BOOL: u8 = 0;
// const TAG_I8: u8 = 1;
// const TAG_U8: u8 = 2;
// const TAG_I16: u8 = 3;
// const TAG_U16: u8 = 4;
// const TAG_I32: u8 = 5;
// const TAG_U32: u8 = 6;
// const TAG_I64: u8 = 7;
// const TAG_U64: u8 = 8;
// const TAG_F64: u8 = 9;
// const TAG_TINY_STR: u8 = 10;
// const TAG_STR: u8 = 11;
// const TAG_TEXT: u8 = 12;
// pub const TAG_POD: u8 = 13;

#[repr(align(8))]
pub struct Padded8<T> {
  value: T,
}

impl<T: Copy> Clone for Padded8<T> {
  #[inline]
  fn clone(&self) -> Self {
    Padded8 {
      value: self.value
    }
  }
}

impl<T: Copy> Copy for Padded8<T> {}

unsafe impl<T: Copy + Clone + Zeroable> Zeroable for Padded8<T> {}
unsafe impl<T: Copy + Clone + Pod> Pod for Padded8<T> {}

struct ArgTag<T>(std::marker::PhantomData<T>);
impl ArgTag<bool> {
  const ARG_TAG: u8 = 0;
}

impl ArgTag<u8> {
  const ARG_TAG: u8 = 1;
}
impl ArgTag<u16> {
  const ARG_TAG: u8 = 2;
}
impl ArgTag<u32> {
  const ARG_TAG: u8 = 3;
}
impl ArgTag<u64> {
  const ARG_TAG: u8 = 4;
}
impl ArgTag<i8> {
  const ARG_TAG: u8 = 5;
}
impl ArgTag<i16> {
  const ARG_TAG: u8 = 6;
}
impl ArgTag<i32> {
  const ARG_TAG: u8 = 7;
}
impl ArgTag<i64> {
  const ARG_TAG: u8 = 8;
}
impl ArgTag<f64> {
  const ARG_TAG: u8 = 9;
}

trait PodTag {
  const ARG_TAG: u8 = 13;
}

impl <T> PodTag for T {}

pub trait Arg: Display + Copy + Clone {
  const ARG_TAG: u8;
}

impl Arg for f64 {
  const ARG_TAG: u8 = 0;
}
impl Arg for u64 {
  const ARG_TAG: u8 = 1;
}
impl Arg for i64 {
  const ARG_TAG: u8 = 2;
}
impl Arg for u32 {
  const ARG_TAG: u8 = 3;
}
impl Arg for i32 {
  const ARG_TAG: u8 = 4;
}
impl Arg for u16 {
  const ARG_TAG: u8 = 5;
}
impl Arg for i16 {
  const ARG_TAG: u8 = 6;
}
impl Arg for u8 {
  const ARG_TAG: u8 = 7;
}
impl Arg for i8 {
  const ARG_TAG: u8 = 8;
}
impl Arg for bool {
  const ARG_TAG: u8 = 9;
}

pub trait UserPod: Display + Copy + Clone {
}

impl <T: UserPod> Arg for T {
  const ARG_TAG: u8 = 13;
}

const fn is_align8<T>() -> bool {
  (size_of::<T>() & 0x7) == 0
}

pub trait IntoArg<T: Arg> {
  fn into_arg(self) -> T;
}

impl IntoArg<u64> for u32 {
  #[inline(always)]
  fn into_arg(self) -> u64 {
    self as u64
  }
}

impl <S: UserPod> IntoArg<S> for S {
  #[inline(always)]
  fn into_arg(self) -> S {
    self
  }
}

pub struct Args0;

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C, packed)]
pub struct Args1<T1: Arg> {
  tag1: u8,
  _pad: [u8; 7],
  arg1: T1,
}

impl <T1: Arg> Args1<T1> {
  const _V1: () = assert!(is_align8::<T1>());

  #[inline]
  pub fn new(arg1: T1) -> Self {
    Args1 {
      tag1: T1::ARG_TAG,
      _pad: [0; 7],
      arg1,
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

impl <T1: Arg, T2: Arg> Args2<T1, T2> {
  const _V1: () = assert!(is_align8::<T1>());
  const _V2: () = assert!(is_align8::<T2>());

  #[inline]
  pub fn new(arg1: T1, arg2: T2) -> Self {
    // println!("tag1 = {} u32 = {}", T1::ARG_TAG, T2::ARG_TAG);
    Args2 {
      tag1: T1::ARG_TAG,
      tag2: T2::ARG_TAG,
      _pad: [0; 6],
      arg1,
      arg2,
    }
  }
}