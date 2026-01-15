use std::io::Write;
use std::{mem, ptr};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PAYLOAD_LEN: usize = 256;

#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn rdtsc() -> u64 {
  unsafe { core::arch::x86_64::_rdtsc() as u64 }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub(crate) fn rdtsc() -> u64 {
  // fallback，仅用于非 x86_64
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_nanos() as u64
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Level {
  Trace = 0,
  Debug = 1,
  Info = 2,
  Warn = 3,
  Error = 4,
}

#[inline(always)]
pub fn enabled(_lvl: Level) -> bool {
  true
}

type LogFn = fn(&mut dyn Write, bytes: &[u8]);

#[repr(C)]
#[derive(Copy, Clone)]
pub struct LogEntry {
  pub tsc: u64,
  pub level: u8,
  pub len: u16,
  pub _pad: [u8; 5],
  pub src_loc: SourceLocation,
  pub func: LogFn,
  pub data: [u8; MAX_PAYLOAD_LEN],
}

impl LogEntry {
  #[inline(always)]
  pub fn from_args<A: Copy>(level: Level, src_loc: SourceLocation, func: LogFn, args: &A) -> Self {
    let sz = size_of::<A>();
    debug_assert!(sz <= MAX_PAYLOAD_LEN);
    let mut e = LogEntry {
      tsc: rdtsc(),
      level: level as u8,
      _pad: [0; 5],
      src_loc,
      func,
      len: sz as u16,
      data: [0u8; MAX_PAYLOAD_LEN],
    };

    unsafe {
      ptr::copy_nonoverlapping(args as *const A as *const u8, e.data.as_mut_ptr(), sz);
    }
    e
  }
}

#[macro_export]
macro_rules! hft_info {
    ($logger:expr, $fmt:literal $(,)?) => {{
        if crate::log::enabled(Level::Info) { __emit0!($logger, Level::Info, $fmt); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr $(,)?) => {{
        if enabled(Level::Info) { __emit1!($logger, Level::Info, $fmt, $a0); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr $(,)?) => {{
        if $crate::log::enabled($crate::log::Level::Info) { $crate::__emit2!($logger, $crate::log::Level::Info, $fmt, $a0, $a1); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr $(,)?) => {{
        if enabled(Level::Info) { __emit3!($logger, Level::Info, $fmt, $a0, $a1, $a2); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr $(,)?) => {{
        if enabled(Level::Info) { __emit4!($logger, Level::Info, $fmt, $a0, $a1, $a2, $a3); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr $(,)?) => {{
        if enabled(Level::Info) { __emit5!($logger, Level::Info, $fmt, $a0, $a1, $a2, $a3, $a4); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr $(,)?) => {{
        if enabled(Level::Info) { __emit6!($logger, Level::Info, $fmt, $a0, $a1, $a2, $a3, $a4, $a5); }
    }};
}


#[derive(Copy, Clone)]
pub struct SourceLocation {
  pub(crate) module_path: &'static str,
  file: &'static str,
  pub(crate) line: u32,
}

impl SourceLocation {
  pub fn __new(module_path: &'static str, file: &'static str, line: u32) -> Self {
    Self {
      module_path,
      file,
      line,
    }
  }

  #[inline(always)]
  pub(crate) fn file_name(&self) -> &'static str {
    let file = if self.file.ends_with(".rs") {
      &self.file[..self.file.len()-3]
    } else {
      self.file
    };
    if let Some(index) = file.rfind(std::path::MAIN_SEPARATOR) {
      &file[index + 1..]
    } else {
      file
    }
  }
}

#[macro_export]
macro_rules! __emit2 {
    ($logger:expr, $lvl:expr, $fmt:literal, $a0:expr, $a1:expr) => {{
      #[inline(never)]
      fn __hft_shim(out: &mut dyn std::io::Write, bytes: &[u8]) {
        let tag1 = bytes[0];
        let tag2 = bytes[1];
        let (arg1, offset) = $crate::args2::decode(tag1, bytes, 8);
        let (arg2, _) = $crate::args2::decode(tag2, bytes, offset);
        let _ = write!(out, $fmt, arg1, arg2);
      }
      let src_loc = $crate::log::SourceLocation::__new(module_path!(), file!(), line!());
      let args2 = $crate::args2::args2($a0, $a1);
      let e = $crate::log::LogEntry::from_args($lvl, src_loc, __hft_shim, &args2);
      $logger.push(e);
    }};
}
