
pub struct MyBytesMut {
  inner: Vec<u8>,
  pos: usize,
}

impl MyBytesMut {
  pub fn with_capacity(capacity: usize) -> Self {
    let inner = std::vec::from_elem(0, capacity);
    let len = 0;
    MyBytesMut {
      inner,
      pos: len,
    }
  }

  #[inline(always)]
  pub fn clear(&mut self) {
    self.pos = 0;
  }

  #[inline(always)]
  pub fn push(&mut self, b: u8) {
    unsafe {
      // self.inner[self.pos] = b;
      *self.inner.get_unchecked_mut(self.pos) = b;
    }
    self.pos += 1;
  }

  #[inline(always)]
  pub fn safe_extend_from_slice(&mut self, src: &[u8]) {
    let new_len = self.pos + src.len();
    if new_len > self.inner.len() {
      self.inner.resize(new_len, 0);
    }
    unsafe {
      std::ptr::copy_nonoverlapping(src.as_ptr(), self.inner[self.pos..].as_mut_ptr(), src.len());
    }
    self.pos = new_len;
  }

  #[inline(always)]
  pub fn extend_from_slice(&mut self, src: &[u8]) {
    let new_len = self.pos + src.len();
    assert!(self.inner.len() > new_len, "buff too small pos={} len={} #src={}", self.pos, self.inner.len(), src.len());
    // if new_len > self.inner.len() {
    //   self.inner.resize(self.inner.len() * 2, 0);
    // }
    unsafe {
      std::ptr::copy_nonoverlapping(src.as_ptr(), self.inner[self.pos..].as_mut_ptr(), src.len());
    }
    self.pos = new_len;
  }

  #[inline(always)]
  pub fn advance(&mut self, len: usize) {
    self.pos += len;
  }

  #[inline(always)]
  pub fn advance2(&mut self, len: usize) -> &[u8] {
    let from = self.pos;
    self.pos += len;
    &self.inner[from..self.pos]
  }

  #[inline(always)]
  pub fn rollback(&mut self, len: usize) {
    assert!(self.pos > len);
    self.pos -= len;
  }

  #[inline(always)]
  pub fn result(&self) -> &[u8] {
    &self.inner[..self.pos]
  }

  #[inline(always)]
  pub fn curr_pos(&self) -> usize {
    self.pos
  }

  #[inline(always)]
  pub fn slice(&self, from: usize, to: usize) -> &[u8] {
    &self.inner[from..to]
  }

  #[inline(always)]
  pub fn unfilled(&mut self) -> &mut [u8] {
    &mut self.inner[self.pos..]
  }
}

impl std::io::Write for MyBytesMut {
  #[inline]
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    self.extend_from_slice(buf);
    Ok(buf.len())
  }

  #[inline]
  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}
