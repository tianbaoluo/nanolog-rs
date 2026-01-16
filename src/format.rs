use std::ptr;

pub(crate) struct TidCache {
  tid_lut: Vec<u8>,
}

impl TidCache {
  pub(crate) const TID_LEN: usize = 4;
  pub fn new(max_tid: usize) -> Self {
    let mut tid_lut = Vec::with_capacity(max_tid * Self::TID_LEN);
    for tid in 0..32 {
      let offset = (tid << 1) as usize;
      tid_lut.extend_from_slice(b"T=");
      tid_lut.extend_from_slice(&DEC_2DIGITS_LUT[offset..offset + 2]);
    }

    TidCache {
      tid_lut,
    }
  }

  pub fn write(&self, tid: usize, buff: &mut [u8]) {
    let offset = (tid << 2);
    unsafe {
      ptr::copy_nonoverlapping(self.tid_lut.as_ptr().add(offset), buff.as_mut_ptr(), Self::TID_LEN);
    }
  }
}

pub struct TimeCache {
  sec: i64,
  buf: [u8; 32], // "MM-DD HH:MM:SS" = 14 bytes
}

impl TimeCache {
  pub(crate) const TIME_LEN: usize = 14;
  pub fn new() -> Self {
    let mut time_cache = TimeCache {
      sec: i64::MAX,
      buf: [0u8; 32],
    };
    unsafe {
      let format = b"00-00 00:00:00";
      ptr::copy_nonoverlapping(format.as_ptr(), time_cache.buf.as_mut_ptr(), format.len());
    }
    time_cache
  }

  pub fn refresh_dt(&mut self, curr_sec: i64, buff: &mut [u8]) {
    if curr_sec == self.sec {
      unsafe {
        ptr::copy_nonoverlapping(self.buf.as_ptr(), buff.as_mut_ptr(), Self::TIME_LEN);
      }
    } else {
      self.sec = curr_sec;
      let (month, day, hour, minute, second) = split_utc(curr_sec);
      unsafe {
        let month_off = (month << 1) as usize;
        ptr::copy_nonoverlapping(DEC_2DIGITS_LUT.as_ptr().add(month_off), self.buf.as_mut_ptr(), 2);
        let day_off = (day << 1) as usize;
        ptr::copy_nonoverlapping(DEC_2DIGITS_LUT.as_ptr().add(day_off), self.buf.as_mut_ptr().add(3), 2);
        let hour_off = (hour << 1) as usize;
        ptr::copy_nonoverlapping(DEC_2DIGITS_LUT.as_ptr().add(hour_off), self.buf.as_mut_ptr().add(6), 2);
        let minute_off = (minute << 1) as usize;
        ptr::copy_nonoverlapping(DEC_2DIGITS_LUT.as_ptr().add(minute_off), self.buf.as_mut_ptr().add(9), 2);
        let second_off = (second << 1) as usize;
        ptr::copy_nonoverlapping(DEC_2DIGITS_LUT.as_ptr().add(second_off), self.buf.as_mut_ptr().add(12), 2);
      }
      unsafe {
        ptr::copy_nonoverlapping(self.buf.as_ptr(), buff.as_mut_ptr(), Self::TIME_LEN);
      }
    }
  }
}

pub(crate) const LEVEL_STRS: &'static [&'static str] = &[
  "trace",
  "debug",
  "\x1b[32minfo\x1b[m ",
  "\x1b[31mwarn\x1b[m ",
  "\x1b[31merror\x1b[m",
  "unk  ",
];

pub fn lut_msus(buf: &mut [u8], ms: usize, us: usize) {
  let rms = ms << 2;
  let rus = us << 2;
  debug_assert!(rms < DEC_4DIGITS_LUT.len());
  debug_assert!(rus < DEC_4DIGITS_LUT.len());
  unsafe {
    let dest = buf.as_mut_ptr();
    ptr::copy_nonoverlapping(DEC_4DIGITS_LUT.as_ptr().add(rms), dest, 4);
    ptr::copy_nonoverlapping(DEC_4DIGITS_LUT.as_ptr().add(rus), dest.add(4), 4);
  }
}

#[inline(always)]
fn civil_from_days(days: i64) -> (u32, u32) {
  // Howard Hinnant: days since 1970-01-01 -> (y,m,d)
  let z = days + 719_468;
  let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
  let doe = z - era * 146_097;                          // [0, 146096]
  let yoe = (doe - doe/1460 + doe/36524 - doe/146096) / 365; // [0, 399]
  // let y = (yoe + era * 400) as i32;
  let doy = doe - (365*yoe + yoe/4 - yoe/100);          // [0, 365]
  let mp = (5*doy + 2) / 153;                           // [0, 11]
  let d = (doy - (153*mp + 2)/5 + 1) as u32;            // [1, 31]
  let m = (mp + if mp < 10 { 3 } else { -9 }) as i32;   // [1, 12]
  // let year = y + if m <= 2 { 1 } else { 0 };
  (m as u32, d)
}

#[inline(always)]
fn split_utc(secs: i64) -> (u32,u32,u32,u32,u32) {
  let days = secs.div_euclid(86_400);
  let sod  = secs.rem_euclid(86_400);
  let (month, day) = civil_from_days(days);
  let hh = (sod / 3600) as u32;
  let mm = ((sod % 3600) / 60) as u32;
  let ss = (sod % 60) as u32;
  (month, day, hh, mm, ss)
}

pub(crate) const DEC_2DIGITS_LUT: [u8; 100 * 2] = *b"\
      0001020304050607080910111213141516171819\
      2021222324252627282930313233343536373839\
      4041424344454647484950515253545556575859\
      6061626364656667686970717273747576777879\
      8081828384858687888990919293949596979899";

const DEC_4DIGITS_LUT: [u8; 1000 * 4] = build_4digit_table();

#[inline(always)]
const fn build_4digit_table() -> [u8; 4_000] {
  let mut table = [0u8; 4_000];
  let mut i = 0;
  while i < 1000 {
    // 生成 "0000".."9999"
    let off = i * 4;
    table[off] = b'.';
    table[off+1] = b'0' + ((i / 100) % 10) as u8;
    table[off+2] = b'0' + ((i / 10) % 10) as u8;
    table[off+3] = b'0' + (i % 10) as u8;
    i += 1;
  }
  table
}