use crate::spsc_var_queue_opt::SpscVarQueueOpt;

pub mod args;
pub mod args2;
pub mod log;
pub mod run_log;
pub(crate) mod spsc;
pub(crate) mod spsc_var_queue_opt;
pub mod run_log2;
pub mod tscns;
pub(crate) mod console_sink;
pub mod format;
pub mod my_bytes_mut;

pub mod spsc_queue {
  pub(crate) type Producer<T> = crate::spsc::Producer<T>;
  pub(crate) type Consumer<T> = crate::spsc::Consumer<T>;

  pub fn spsc_queue<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    crate::spsc::ring_buffer(capacity)
  }
}

pub type StagingBuffer = SpscVarQueueOpt<1024>;