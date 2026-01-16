use core::cell::UnsafeCell;
use core::mem::{align_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering, compiler_fence};

pub const BLOCK_SIZE: usize = 64;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MsgHeader {
  /// total bytes including header; 0 means "rewind marker"
  pub size: u32,
  pub level: u32,
  pub tsc: u64,
  pub func: u64,
}
pub const MSG_HEADER_SIZE: usize = size_of::<MsgHeader>();

#[repr(C, align(64))]
#[derive(Copy, Clone)]
struct Block {
  header: MsgHeader,
  bytes: [u8; BLOCK_SIZE - MSG_HEADER_SIZE],
}

#[inline(always)]
const fn is_pow2(x: usize) -> bool { x != 0 && (x & (x - 1)) == 0 }

#[inline(always)]
fn div_ceil(a: usize, b: usize) -> usize { (a + b - 1) / b }

pub struct SpscVarQueueOpt<const BLK_CNT: usize> {
  blk: UnsafeCell<[Block; BLK_CNT]>,

  // producer-owned (consumer reads)
  writing_idx: AtomicU32,
  written_idx: AtomicU32,

  // consumer-owned (producer reads)
  read_idx: AtomicU32,

  // producer cache
  read_idx_cache: UnsafeCell<u32>,
}

unsafe impl<const BLK_CNT: usize> Sync for SpscVarQueueOpt<BLK_CNT> {}

impl<const BLK_CNT: usize> SpscVarQueueOpt<BLK_CNT> {
  pub fn new() -> Self {
    assert!(is_pow2(BLK_CNT), "BLK_CNT must be power of two");
    assert!(BLOCK_SIZE % align_of::<MsgHeader>() == 0);
    assert!(MSG_HEADER_SIZE <= BLOCK_SIZE);

    let zero_block = Block {
      header: MsgHeader { size: 0, level: 0, tsc: 0, func: 0 },
      bytes: [0u8; BLOCK_SIZE - MSG_HEADER_SIZE],
    };

    Self {
      blk: UnsafeCell::new([zero_block; BLK_CNT]),
      writing_idx: AtomicU32::new(0),
      written_idx: AtomicU32::new(0),
      read_idx: AtomicU32::new(0),
      read_idx_cache: UnsafeCell::new(0),
    }
  }

  #[inline(always)]
  fn mask() -> u32 { (BLK_CNT as u32) - 1 }

  #[inline(always)]
  fn blk_ptr(&self) -> *mut Block {
    unsafe { (*self.blk.get()).as_mut_ptr() }
  }

  pub fn split(&self) -> (Producer<'_, BLK_CNT>, Consumer<'_, BLK_CNT>) {
    (Producer { q: self }, Consumer { q: self })
  }
}

/// Producer handle (single thread)
pub struct Producer<'a, const BLK_CNT: usize> { pub q: &'a SpscVarQueueOpt<BLK_CNT> }

/// Consumer handle (single thread)
pub struct Consumer<'a, const BLK_CNT: usize> { pub q: &'a SpscVarQueueOpt<BLK_CNT> }

impl<'a, const BLK_CNT: usize> Producer<'a, BLK_CNT> {
  /// Allocate payload_len bytes (excluding header).
  /// Returns (hdr_ptr, payload_ptr, payload_cap_bytes, total_bytes, blk_sz)
  ///
  /// payload_cap_bytes == blk_sz*BLOCK_SIZE - MSG_HEADER_SIZE  (enough to write payload_len)
  #[inline(always)]
  pub fn try_alloc(&self, payload_len: usize)
                   -> Option<(*mut MsgHeader, *mut u8, usize, u32, u32)>
  {
    let total_bytes = payload_len.checked_add(MSG_HEADER_SIZE)?;
    let blk_sz = div_ceil(total_bytes, BLOCK_SIZE) as u32;

    let mut write_idx = self.q.writing_idx.load(Ordering::Relaxed);

    // blocks remaining to ring end
    let pad = (BLK_CNT as u32) - (write_idx & SpscVarQueueOpt::<BLK_CNT>::mask());
    let rewind = blk_sz > pad;
    let needed = blk_sz + if rewind { pad } else { 0 };

    // need read_idx <= write_idx + needed - BLK_CNT
    let min_read_idx = write_idx.wrapping_add(needed).wrapping_sub(BLK_CNT as u32);

    let ric = unsafe { &mut *self.q.read_idx_cache.get() };
    if (*ric as i32) < (min_read_idx as i32) {
      let fresh = self.q.read_idx.load(Ordering::Acquire);
      *ric = fresh;
      if (fresh as i32) < (min_read_idx as i32) {
        return None;
      }
    }

    let blk = self.q.blk_ptr();

    if rewind {
      // write rewind marker at current block
      let cur = unsafe { blk.add((write_idx & SpscVarQueueOpt::<BLK_CNT>::mask()) as usize) };
      unsafe { ptr::write_volatile(&mut (*cur).header.size, 0) };
      compiler_fence(Ordering::Release);

      write_idx = write_idx.wrapping_add(pad);
      self.q.writing_idx.store(write_idx, Ordering::Relaxed);
    }

    let cur = unsafe { blk.add((write_idx & SpscVarQueueOpt::<BLK_CNT>::mask()) as usize) };
    let hdr_ptr = unsafe { &mut (*cur).header as *mut MsgHeader };

    // contiguous region start pointer at header (first block)
    let base_ptr = hdr_ptr as *mut u8;
    let payload_ptr = unsafe { base_ptr.add(MSG_HEADER_SIZE) };

    // reserve blocks (not published)
    let new_write = write_idx.wrapping_add(blk_sz);
    self.q.writing_idx.store(new_write, Ordering::Relaxed);

    let payload_cap = (blk_sz as usize) * BLOCK_SIZE - MSG_HEADER_SIZE;
    Some((hdr_ptr, payload_ptr, payload_cap, total_bytes as u32, blk_sz))
  }

  /// Publish after writing header fields (except size) + payload.
  #[inline(always)]
  pub unsafe fn commit(&self, hdr: *mut MsgHeader, total_bytes_including_header: u32) {
    // publish size last
    ptr::write_volatile(&mut (*hdr).size, total_bytes_including_header);
    compiler_fence(Ordering::Release);

    let w = self.q.writing_idx.load(Ordering::Relaxed);
    self.q.written_idx.store(w, Ordering::Release);
  }
}

impl<'a, const BLK_CNT: usize> Consumer<'a, BLK_CNT> {
  /// Peek front message. Returns (hdr_ptr, payload_ptr, total_bytes).
  #[inline(always)]
  pub fn front(&self) -> Option<(*const MsgHeader, *const u8, u32)> {
    let mut r = self.q.read_idx.load(Ordering::Relaxed);
    let w = self.q.written_idx.load(Ordering::Acquire);
    if r == w { return None; }

    let blk = self.q.blk_ptr();
    loop {
      let cur = unsafe { blk.add((r & SpscVarQueueOpt::<BLK_CNT>::mask()) as usize) };
      let sz = unsafe { ptr::read_volatile(&(*cur).header.size) };

      if sz == 0 {
        // rewind
        let pad = (BLK_CNT as u32) - (r & SpscVarQueueOpt::<BLK_CNT>::mask());
        r = r.wrapping_add(pad);
        self.q.read_idx.store(r, Ordering::Relaxed);
        if r == w { return None; }
        continue;
      }

      let hdr_ptr = unsafe { &(*cur).header as *const MsgHeader };
      let base_ptr = hdr_ptr as *const u8;
      let payload_ptr = unsafe { base_ptr.add(MSG_HEADER_SIZE) };
      return Some((hdr_ptr, payload_ptr, sz));
    }
  }

  #[inline(always)]
  pub fn pop(&self) {
    let r = self.q.read_idx.load(Ordering::Relaxed);

    let blk = self.q.blk_ptr();
    let cur = unsafe { blk.add((r & SpscVarQueueOpt::<BLK_CNT>::mask()) as usize) };
    let sz = unsafe { ptr::read_volatile(&(*cur).header.size) };
    debug_assert!(sz != 0);

    let blk_sz = div_ceil(sz as usize, BLOCK_SIZE) as u32;
    let new_r = r.wrapping_add(blk_sz);

    self.q.read_idx.store(new_r, Ordering::Release);
  }
}