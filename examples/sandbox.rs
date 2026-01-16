use hft_log_demo::format::TimeCache;

fn main() {
  if true {
    let curr_sec = now_sec();
    let mut time_cache = TimeCache::new();
    println!("now {}", String::from_utf8_lossy(time_cache.refresh_dt(curr_sec)));
    println!("now {}", String::from_utf8_lossy(time_cache.refresh_dt(curr_sec + 10)));
    // return;
  }

  // [t01] [t99]
  let mut tid_table = Vec::with_capacity(512);
  for tid in 0..32 {
    let offset = (tid << 1) as usize;
    tid_table.extend_from_slice(b"T=");
    tid_table.extend_from_slice(&DEC_2DIGITS_LUT[offset..offset+2]);
    // tid_table.extend_from_slice(b"]");
    // println!("{}: {}", tid, String::from_utf8_lossy(&DEC_2DIGITS_LUT[offset..offset+2]));
  }
  for tid in 0..32 {
    let offset = (tid << 2) as usize;
    println!("{}: {}", tid, String::from_utf8_lossy(&tid_table[offset..offset+4]));
  }
}

#[inline(always)]
pub fn now_sec() -> i64 {
  unsafe {
    let mut time: libc::timespec = std::mem::zeroed();
    if libc::clock_gettime(libc::CLOCK_REALTIME, &mut time) == -1 {
      unreachable!("Call libc::clock_gettime(libc::CLOCK_REALTIME, &mut time) error: {:?}", std::io::Error::last_os_error());
    }
    time.tv_sec as i64
  }
}

const DEC_2DIGITS_LUT: [u8; 100 * 2] = *b"\
      0001020304050607080910111213141516171819\
      2021222324252627282930313233343536373839\
      4041424344454647484950515253545556575859\
      6061626364656667686970717273747576777879\
      8081828384858687888990919293949596979899";