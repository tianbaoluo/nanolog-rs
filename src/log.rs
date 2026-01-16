use std::io::Write;
use std::{io, mem, ptr};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PAYLOAD_LEN: usize = 256;

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub fn rdtsc() -> u64 {
  unsafe { core::arch::x86_64::_rdtsc() as u64 }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub fn rdtsc() -> u64 {
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

pub(crate) type LogFn = fn(&mut dyn Write, u32, bytes: &[u8]) -> io::Result<()>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct LogEntry {
  pub tsc: u64,
  pub level: u64,
  // pub len: u16,
  // pub _pad: [u8; 7],
  pub func: LogFn,
  pub data: [u8; MAX_PAYLOAD_LEN],
}

impl LogEntry {
  #[inline(always)]
  pub fn from_args<A: Copy>(level: Level, func: LogFn, args: &A) -> Self {
    let sz = size_of::<A>();
    debug_assert!(sz <= MAX_PAYLOAD_LEN);
    let mut log_entry = LogEntry {
      tsc: 0, //rdtsc(),
      level: level as u8 as u64,
      // len: sz as u16,
      // _pad: [0; 7],
      func,
      data: [0u8; MAX_PAYLOAD_LEN],
    };

    unsafe {
      ptr::copy_nonoverlapping(args as *const A as *const u8, log_entry.data.as_mut_ptr(), sz);
    }
    log_entry
  }

  #[inline(always)]
  pub fn mut_from_args<A: Copy>(&mut self, level: Level, func: LogFn, args: &A) {
    let sz = size_of::<A>();
    debug_assert!(sz <= MAX_PAYLOAD_LEN);
    self.tsc = 0; // rdtsc();
    self.level = level as u8 as u64;
    self.func = func;
    // let mut log_entry = LogEntry {
    //   tsc: 0, //rdtsc(),
    //   level: level as u8 as u64,
    //   // len: sz as u16,
    //   _pad: [0; 7],
    //   func,
    //   data: [0u8; MAX_PAYLOAD_LEN],
    // };

    unsafe {
      ptr::copy_nonoverlapping(args as *const A as *const u8, self.data.as_mut_ptr(), sz);
    }
    // log_entry
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
        //if $crate::log::enabled($crate::log::Level::Info) { $crate::__emit2!($logger, $crate::log::Level::Info, $fmt, $a0, $a1); }
        $crate::__emit2!($logger, $crate::log::Level::Info, $fmt, $a0, $a1)
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
      fn __hft_shim(out: &mut dyn std::io::Write, tid: u32, bytes: &[u8]) -> std::io::Result<()> {
        let src_loc = $crate::log::SourceLocation::__new(module_path!(), file!(), line!());
        $crate::log::write_loc_tid(out, src_loc, tid)?;
        let tag1 = bytes[0];
        let tag2 = bytes[1];
        let (arg1, offset) = $crate::args2::decode(tag1, bytes, 8);
        let (arg2, _) = $crate::args2::decode(tag2, bytes, offset);
        write!(out, $fmt, arg1, arg2)
      }
      let args2 = $crate::args2::args2($a0, $a1);
      $logger.publish_args($lvl, __hft_shim, &args2)
      // $logger.push_write(|log_entry| log_entry.mut_from_args($lvl, __hft_shim, &args2))
      //let e = $crate::log::LogEntry::from_args($lvl, __hft_shim, &args2);
      //std::hint::black_box(e);
      // $logger.push(e);
    }};
}

#[inline(always)]
pub fn write_loc_tid(out: &mut dyn std::io::Write, src_loc: SourceLocation, tid: u32) -> io::Result<()> {
  out.write_all(src_loc.module_path.as_bytes())?;
  out.write_all(b"::")?;
  out.write_all(src_loc.file_name().as_bytes())?;
  write!(out, "#{} {}", src_loc.line, tid)?;
  out.write_all(b"] ")?;
  Ok(())
}
