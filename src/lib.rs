pub mod args;
pub mod args2;
pub mod log;
pub mod run_log;
pub(crate) mod spsc;

pub mod spsc_queue {
  pub(crate) type Producer<T> = crate::spsc::Producer<T>;
  pub(crate) type Consumer<T> = crate::spsc::Consumer<T>;

  pub fn spsc_queue<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    crate::spsc::ring_buffer(capacity)
  }
}