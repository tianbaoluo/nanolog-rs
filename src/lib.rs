pub mod args;
pub mod args2;
pub mod log;
pub mod run_log;

pub mod spsc_queue {
  pub(crate) type Producer<T> = nexus_queue::spsc::Producer<T>;
  pub(crate) type Consumer<T> = nexus_queue::spsc::Consumer<T>;

  pub fn spsc_queue<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    nexus_queue::spsc::ring_buffer(capacity)
  }
}