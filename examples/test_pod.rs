use std::fmt::Display;

fn is_copy<T: Copy>(f: T) -> T {
  f
}

fn main() {
  let args3 = Args3 {
    arg1: 23_u64,
    arg2: 12_u32,
    arg3: 4443_u32,
  };

  let ax = move || { println!("mix p1 x={0} s={1}", args3.arg1, args3.arg2); };
  println!("size-of(ax) = {} type: {}", size_of_val(&ax), std::any::type_name_of_val(&ax));
  let bx = is_copy(ax);
  println!("size-of(ax) = {} type: {}", size_of_val(&bx), std::any::type_name_of_val(&bx));

  let args = format_args!("mix p1 x={0} s={1}", args3.arg1, args3.arg2);

  let mut log_fns: Vec<dyn LogArgs> = Vec::with_capacity(8);
  {
    #[derive(Copy, Clone)]
    struct __LogArgs<T1: Copy + Display, T2: Copy + Display, T3: Copy + Display>(Args3<T1,T2,T3>);
    impl <T1: Copy + Display, T2: Copy + Display, T3: Copy + Display> LogArgs for __LogArgs<T1, T2, T3> {
      fn println(&self) {
        println!("mix p1 x={0} s={1}", self.0.arg1, self.0.arg2);
      }
    }

    let log_entry = __LogArgs(args3);
    log_fns.push(log_entry);
  }

  {
    #[derive(Copy, Clone)]
    struct __LogArgs<T1: Copy + Display, T2: Copy + Display, T3: Copy + Display>(Args3<T1,T2,T3>);
    impl <T1: Copy + Display, T2: Copy + Display, T3: Copy + Display> LogArgs for __LogArgs<T1, T2, T3> {
      fn println(&self) {
        println!("mix p2 x={0} s={1}", self.0.arg1, self.0.arg2);
      }
    }

    let log_entry = __LogArgs(args3);
    log_fns.push((&log_entry) as _);
  }

  for log_fn in log_fns.into_iter() {
    unsafe {
      (*log_fn).println();
    }
  }
}

pub trait LogArgs: Sized {
  fn println(&self);
}

#[derive(Copy, Clone)]
#[repr(C)]
struct Args3<T1: Copy + Display, T2: Copy + Display, T3: Copy + Display> {
  arg1: T1,
  arg2: T2,
  arg3: T3,
}
