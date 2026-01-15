use crossbeam_channel::{Receiver, Sender};
use std::cell::{Cell, UnsafeCell};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt;
use std::io::{self, Write};
use std::mem::{self, MaybeUninit};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ENTRY_BYTES: usize = 256;
const STR_TRUNC: usize = 64;

#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() as u64 }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
fn rdtsc() -> u64 {
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
fn enabled(_lvl: Level) -> bool {
    true
}

#[inline(always)]
fn level_str(l: u8) -> &'static str {
    match l {
        0 => "trace",
        1 => "debug",
        2 => "\x1b[32minfo\x1b[m ",
        3 => "\x1b[31mwarn\x1b[m ",
        4 => "\x1b[31merror\x1b[m",
        _ => "unk  ",
    }
}

// =============================
// InlineStr
// =============================
#[repr(C)]
#[derive(Copy, Clone)]
pub struct InlineStr<const N: usize> {
    len: u16,
    buf: [u8; N],
}

impl<const N: usize> InlineStr<N> {
    #[inline(always)]
    pub fn new(s: &str) -> Self {
        let b = s.as_bytes();
        let n = b.len().min(N);
        let mut out = InlineStr {
            len: n as u16,
            buf: [0u8; N],
        };
        out.buf[..n].copy_from_slice(&b[..n]);
        out
    }

    #[inline(always)]
    pub fn as_str(&self) -> &str {
        let n = self.len as usize;
        std::str::from_utf8(&self.buf[..n]).unwrap_or("<utf8-trunc>")
    }
}

impl<const N: usize> fmt::Display for InlineStr<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
impl<const N: usize> fmt::Debug for InlineStr<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// =============================
// HeavyRef: ptr/hash
// =============================
#[repr(C)]
#[derive(Copy, Clone)]
pub struct HeavyRef {
    ptr: u64,
    hash: u64,
}

#[inline(always)]
fn cheap_hash64(mut x: u64) -> u64 {
    x ^= 0x9E3779B97F4A7C15;
    x ^= x >> 33;
    x = x.wrapping_mul(0xC2B2AE3D27D4EB4F);
    x ^= x >> 29;
    x
}

impl HeavyRef {
    #[inline(always)]
    pub fn from_ptr<T: ?Sized>(p: *const T) -> Self {
        let u = p as *const () as usize as u64;
        Self {
            ptr: u,
            hash: cheap_hash64(u),
        }
    }
}

impl fmt::Display for HeavyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "obj@0x{:x}#{}", self.ptr, self.hash)
    }
}
impl fmt::Debug for HeavyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// =============================
// Ptr wrapper (avoid IntoArg overlap with &str)
// =============================
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Ptr<T: ?Sized>(*const T);

#[inline(always)]
pub fn ptr<T: ?Sized>(v: &T) -> Ptr<T> {
    Ptr(v as *const T)
}

// =============================
// Arg (uniform snap type)
// =============================
#[repr(C)]
#[derive(Copy, Clone)]
pub enum Arg {
    I64(i64),
    U64(u64),
    F64(f64),
    Bool(bool),
    Char(char),
    Str(InlineStr<STR_TRUNC>),
    Heavy(HeavyRef),
    None,
}

impl fmt::Display for Arg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arg::I64(v) => write!(f, "{v}"),
            Arg::U64(v) => write!(f, "{v}"),
            Arg::F64(v) => write!(f, "{v}"),
            Arg::Bool(v) => write!(f, "{v}"),
            Arg::Char(v) => write!(f, "{v}"),
            Arg::Str(v) => fmt::Display::fmt(v, f),
            Arg::Heavy(v) => fmt::Display::fmt(v, f),
            Arg::None => Ok(()),
        }
    }
}
impl fmt::Debug for Arg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arg::I64(v) => write!(f, "I64({v})"),
            Arg::U64(v) => write!(f, "U64({v})"),
            Arg::F64(v) => write!(f, "F64({v})"),
            Arg::Bool(v) => write!(f, "Bool({v})"),
            Arg::Char(v) => write!(f, "Char({v:?})"),
            Arg::Str(v) => write!(f, "Str({:?})", v.as_str()),
            Arg::Heavy(v) => write!(f, "{v}"),
            Arg::None => write!(f, "None"),
        }
    }
}

// =============================
// IntoArg
// =============================
pub trait IntoArg {
    fn into_arg(self) -> Arg;
}

macro_rules! impl_into_i64 {
    ($t:ty) => {
        impl IntoArg for $t {
            #[inline(always)]
            fn into_arg(self) -> Arg {
                Arg::I64(self as i64)
            }
        }
    };
}
macro_rules! impl_into_u64 {
    ($t:ty) => {
        impl IntoArg for $t {
            #[inline(always)]
            fn into_arg(self) -> Arg {
                Arg::U64(self as u64)
            }
        }
    };
}

impl_into_i64!(i8);
impl_into_i64!(i16);
impl_into_i64!(i32);
impl_into_i64!(i64);
impl_into_i64!(isize);

impl_into_u64!(u8);
impl_into_u64!(u16);
impl_into_u64!(u32);
impl_into_u64!(u64);
impl_into_u64!(usize);

impl IntoArg for f32 {
    #[inline(always)]
    fn into_arg(self) -> Arg {
        Arg::F64(self as f64)
    }
}
impl IntoArg for f64 {
    #[inline(always)]
    fn into_arg(self) -> Arg {
        Arg::F64(self)
    }
}
impl IntoArg for bool {
    #[inline(always)]
    fn into_arg(self) -> Arg {
        Arg::Bool(self)
    }
}
impl IntoArg for char {
    #[inline(always)]
    fn into_arg(self) -> Arg {
        Arg::Char(self)
    }
}
impl<'a> IntoArg for &'a str {
    #[inline(always)]
    fn into_arg(self) -> Arg {
        Arg::Str(InlineStr::<STR_TRUNC>::new(self))
    }
}
impl IntoArg for String {
    #[inline(always)]
    fn into_arg(self) -> Arg {
        Arg::Str(InlineStr::<STR_TRUNC>::new(self.as_str()))
    }
}
impl<'a> IntoArg for &'a String {
    #[inline(always)]
    fn into_arg(self) -> Arg {
        Arg::Str(InlineStr::<STR_TRUNC>::new(self.as_str()))
    }
}
impl<T: ?Sized> IntoArg for Ptr<T> {
    #[inline(always)]
    fn into_arg(self) -> Arg {
        Arg::Heavy(HeavyRef::from_ptr(self.0))
    }
}

// =============================
// Thin-closure LogEntry
// =============================
type LogFn = fn(&mut dyn Write, *const u8);

#[repr(C)]
#[derive(Copy, Clone)]
pub struct LogEntry<const BYTES: usize> {
    pub tsc: u64,
    pub level: u8,
    pub _pad: [u8; 7],
    pub src_loc: SourceLocation,
    // pub site: &'static str, // 优化#3：宏生成的 module::file#line
    pub func: LogFn,
    pub len: u16,
    pub _pad2: [u8; 6],
    pub data: [u8; BYTES],
}

impl<const BYTES: usize> LogEntry<BYTES> {
    #[inline(always)]
    fn from_args<A: Copy>(level: Level, src_loc: SourceLocation, func: LogFn, args: &A) -> Self {
        let mut e = LogEntry {
            tsc: rdtsc(),
            level: level as u8,
            _pad: [0; 7],
            src_loc,
            // site,
            func,
            len: mem::size_of::<A>() as u16,
            _pad2: [0; 6],
            data: [0u8; BYTES],
        };
        let sz = mem::size_of::<A>();
        debug_assert!(sz <= BYTES);
        unsafe {
            ptr::copy_nonoverlapping(args as *const A as *const u8, e.data.as_mut_ptr(), sz);
        }
        e
    }
}

// =============================
// nexus_queue wrapper
// =============================
pub mod spsc_queue {
    pub(crate) type Producer<T> = nexus_queue::spsc::Producer<T>;
    pub(crate) type Consumer<T> = nexus_queue::spsc::Consumer<T>;

    pub fn spsc_queue<T>(capacity: usize) -> (Producer<T>, Consumer<T>) {
        nexus_queue::spsc::ring_buffer(capacity)
    }
}

// =============================
// Linux tid + localtime_r helpers
// =============================
#[cfg(target_os = "linux")]
#[inline(always)]
fn get_tid() -> u32 {
    unsafe { libc::syscall(libc::SYS_gettid) as u32 }
}

#[cfg(target_os = "macos")]
#[inline(always)]
fn get_tid() -> u32 {
    // macOS: use pthread_threadid_np -> u64
    unsafe {
        let mut tid64: u64 = 0;
        // 0 means "current thread"
        libc::pthread_threadid_np(0, &mut tid64 as *mut u64);
        // truncate to u32 for header (够用了)
        tid64 as u32
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
#[inline(always)]
fn get_tid() -> u32 {
    // 其它 Unix：先给一个稳定但不 syscall-heavy 的 fallback
    // （你要更严格的 OS tid，我们再按平台补）
    std::thread::current().id().as_u64().get() as u32
}

#[cfg(not(unix))]
#[inline(always)]
fn get_tid() -> u32 {
    0
}

#[inline(always)]
fn two_digits(dst: &mut [u8], x: u32) {
    dst[0] = b'0' + ((x / 10) as u8);
    dst[1] = b'0' + ((x % 10) as u8);
}
#[inline(always)]
fn three_digits(dst: &mut [u8], x: u32) {
    dst[0] = b'0' + ((x / 100) as u8);
    dst[1] = b'0' + (((x / 10) % 10) as u8);
    dst[2] = b'0' + ((x % 10) as u8);
}

// =============================
// TSC -> epoch_ns mapping + prefix cache (优化#1)
// =============================
pub struct TscClock {
    base_tsc: u64,
    base_epoch_ns: u64,
    hz: f64,
}

impl TscClock {
    pub fn calibrate() -> Self {
        // base
        let base_epoch_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        // hz
        let t0 = rdtsc();
        let s0 = std::time::Instant::now();
        std::thread::sleep(Duration::from_millis(10));
        let t1 = rdtsc();
        let dt_ns = s0.elapsed().as_nanos() as f64;
        let hz = (t1 - t0) as f64 * 1e9 / dt_ns;

        Self {
            base_tsc: t1,
            base_epoch_ns,
            hz,
        }
    }

    #[inline(always)]
    pub fn tsc_to_epoch_ns(&self, tsc: u64) -> u64 {
        let d_cycles = tsc.wrapping_sub(self.base_tsc) as f64;
        let d_ns = d_cycles * 1e9 / self.hz;
        self.base_epoch_ns.wrapping_add(d_ns.max(0.0) as u64)
    }
}

struct PrefixCache {
    sec: u64,
    buf: [u8; 32], // "MM-DD HH:MM:SS" = 14 bytes
    len: usize,
}

impl PrefixCache {
    fn new() -> Self {
        Self {
            sec: u64::MAX,
            buf: [0u8; 32],
            len: 0,
        }
    }

    #[inline(never)]
    fn refresh(&mut self, sec: u64) {
        self.sec = sec;

        // localtime_r
        #[cfg(unix)]
        unsafe {
            let mut t: libc::tm = std::mem::zeroed();
            let mut tt: libc::time_t = sec as libc::time_t;
            libc::localtime_r(&tt as *const libc::time_t, &mut t as *mut libc::tm);

            let mon = (t.tm_mon + 1) as u32;
            let mday = t.tm_mday as u32;
            let hour = t.tm_hour as u32;
            let min = t.tm_min as u32;
            let ssec = t.tm_sec as u32;

            // "MM-DD HH:MM:SS"
            let b = &mut self.buf;
            two_digits(&mut b[0..2], mon);
            b[2] = b'-';
            two_digits(&mut b[3..5], mday);
            b[5] = b' ';
            two_digits(&mut b[6..8], hour);
            b[8] = b':';
            two_digits(&mut b[9..11], min);
            b[11] = b':';
            two_digits(&mut b[12..14], ssec);

            self.len = 14;
        }

        #[cfg(not(unix))]
        {
            // fallback: just show sec
            self.len = 0;
        }
    }
}

// =============================
// Register message (优化#2：带 tid)
// =============================
struct RegMsg {
    cons: spsc_queue::Consumer<LogEntry<ENTRY_BYTES>>,
    tid: u32,
}

// =============================
// Logger handle + TLS producer init
// =============================
pub struct LoggerHandle {
    reg_tx: Sender<RegMsg>,
    capacity: usize,
}

impl Clone for LoggerHandle {
    fn clone(&self) -> Self {
        Self {
            reg_tx: self.reg_tx.clone(),
            capacity: self.capacity,
        }
    }
}

struct TlsProd {
    inited: Cell<bool>,
    prod: UnsafeCell<MaybeUninit<spsc_queue::Producer<LogEntry<ENTRY_BYTES>>>>,
    tid: Cell<u32>,
}

unsafe impl Sync for TlsProd {}

static NEXT_TID: AtomicU32 = AtomicU32::new(1);

impl TlsProd {
    #[inline(always)]
    fn get_mut(&self, logger: &LoggerHandle) -> &mut spsc_queue::Producer<LogEntry<ENTRY_BYTES>> {
        if !self.inited.get() {
            let (prod, cons) = spsc_queue::spsc_queue::<LogEntry<ENTRY_BYTES>>(logger.capacity);
            let tid = NEXT_TID.fetch_add(1, Ordering::Relaxed); //get_tid();
            self.tid.set(tid);

            let _ = logger.reg_tx.send(RegMsg { cons, tid });

            unsafe { (*self.prod.get()).write(prod) };
            self.inited.set(true);
        }
        unsafe { (*self.prod.get()).assume_init_mut() }
    }
}

thread_local! {
    static TLS_PROD: TlsProd = TlsProd {
        inited: Cell::new(false),
        prod: UnsafeCell::new(MaybeUninit::uninit()),
        tid: Cell::new(0),
    };
}

impl LoggerHandle {
    #[inline(always)]
    pub fn push(&self, e: LogEntry<ENTRY_BYTES>) {
        TLS_PROD.with(|tls| {
            let p = tls.get_mut(self);
            let _ = p.push(e); // 满了就丢；你可以加 dropped 计数
        });
    }
}

// =============================
// Logger thread: collect consumers + K-way heap merge by tsc
// =============================
struct QState {
    cons: spsc_queue::Consumer<LogEntry<ENTRY_BYTES>>,
    head: Option<LogEntry<ENTRY_BYTES>>,
    tid: u32,
}

struct LoggerThread {
    reg_rx: Receiver<RegMsg>,
    qs: Vec<QState>,
    heap: BinaryHeap<Reverse<(u64, usize)>>, // (tsc, qid)
    empty: Vec<usize>,
    empty_cursor: usize,
    clock: TscClock,
    prefix: PrefixCache,
}

impl LoggerThread {
    fn new(reg_rx: Receiver<RegMsg>) -> Self {
        Self {
            reg_rx,
            qs: Vec::new(),
            heap: BinaryHeap::new(),
            empty: Vec::new(),
            empty_cursor: 0,
            clock: TscClock::calibrate(),
            prefix: PrefixCache::new(),
        }
    }

    fn add_consumer(&mut self, msg: RegMsg) {
        let mut cons = msg.cons;
        let mut st = QState {
            cons,
            head: None,
            tid: msg.tid,
        };
        let qid = self.qs.len();
        if let Some(e) = st.cons.pop() {
            let t = e.tsc;
            st.head = Some(e);
            self.qs.push(st);
            self.heap.push(Reverse((t, qid)));
        } else {
            self.qs.push(st);
            self.empty.push(qid);
        }
    }

    #[inline(always)]
    fn refill_head(&mut self, qid: usize) {
        if self.qs[qid].head.is_none() {
            if let Some(e) = self.qs[qid].cons.pop() {
                let t = e.tsc;
                self.qs[qid].head = Some(e);
                self.heap.push(Reverse((t, qid)));
            } else {
                self.empty.push(qid);
            }
        }
    }

    #[inline(always)]
    fn scan_empty_budget(&mut self, budget: usize) {
        let mut b = 0;
        while b < budget && !self.empty.is_empty() {
            let len = self.empty.len();
            let idx = self.empty_cursor % len;
            let qid = self.empty[idx];

            if self.qs[qid].head.is_none() {
                if let Some(e) = self.qs[qid].cons.pop() {
                    let t = e.tsc;
                    self.qs[qid].head = Some(e);
                    self.heap.push(Reverse((t, qid)));
                    self.empty.swap_remove(idx);
                } else {
                    self.empty_cursor = self.empty_cursor.wrapping_add(1);
                }
            } else {
                self.empty.swap_remove(idx);
            }

            b += 1;
        }
    }

    #[inline(always)]
    fn write_header(&mut self, out: &mut dyn Write, e: &LogEntry<ENTRY_BYTES>, tid: u32) -> io::Result<()> {
        // tsc -> epoch_ns
        let epoch_ns = self.clock.tsc_to_epoch_ns(e.tsc);
        let sec = epoch_ns / 1_000_000_000;
        let sub = (epoch_ns % 1_000_000_000) as u32;
        let ms = sub / 1_000_000;
        let us = (sub / 1_000) % 1000;

        // per-second prefix cache: "MM-DD HH:MM:SS"
        if self.prefix.sec != sec {
            self.prefix.refresh(sec);
        }

        // [MM-DD HH:MM:SS.mmm.uuu level site tid]
        out.write_all(b"[")?;
        out.write_all(&self.prefix.buf[..self.prefix.len])?;
        out.write_all(b".")?;
        let mut tmp = [0u8; 3];
        three_digits(&mut tmp, ms);
        out.write_all(&tmp)?;
        out.write_all(b".")?;
        three_digits(&mut tmp, us);
        out.write_all(&tmp)?;
        out.write_all(b" ")?;
        out.write_all(level_str(e.level).as_bytes())?;
        out.write_all(b" ")?;
        let src_loc = e.src_loc;
        out.write_all(src_loc.module_path.as_bytes())?;
        out.write_all(b"::")?;
        out.write_all(src_loc.file_name().as_bytes())?;
        write!(out, "#{} {}", src_loc.line, tid)?;
        // out.write_all(e.site.as_bytes())?;
        // out.write_all(b" ")?;
        // tid
        // 写数字用 itoa 更快，但这里先用 write!（logger 线程，且每条一次）
        // write!(out, "{}", tid)?;
        out.write_all(b"] ")?;
        Ok(())
    }

    fn run(mut self) -> io::Result<()> {
        let stdout = io::stdout();
        let mut out = io::BufWriter::new(stdout.lock());

        loop {
            while let Ok(msg) = self.reg_rx.try_recv() {
                self.add_consumer(msg);
            }

            // 小预算扫描空队列（兜底）
            self.scan_empty_budget(4);

            if let Some(Reverse((_t, qid))) = self.heap.pop() {
                let e = self.qs[qid].head.take().unwrap();
                let tid = self.qs[qid].tid;

                self.write_header(&mut out, &e, tid)?;
                (e.func)(&mut out, e.data.as_ptr());
                out.write_all(b"\n")?;

                self.refill_head(qid);
            } else {
                std::thread::yield_now();
            }
        }
    }
}

// =============================
// init_logger
// =============================
fn init_logger(capacity: usize) -> LoggerHandle {
    let (reg_tx, reg_rx) = crossbeam_channel::unbounded();

    std::thread::spawn(move || {
        let lt = LoggerThread::new(reg_rx);
        let _ = lt.run();
    });

    LoggerHandle { reg_tx, capacity }
}

// =============================
// Args0..Args6
// =============================
#[repr(C)]
#[derive(Copy, Clone)]
struct Args0;

#[repr(C)]
#[derive(Copy, Clone)]
struct Args1 {
    a0: Arg,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Args2 {
    a0: Arg,
    a1: Arg,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Args3 {
    a0: Arg,
    a1: Arg,
    a2: Arg,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Args4 {
    a0: Arg,
    a1: Arg,
    a2: Arg,
    a3: Arg,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Args5 {
    a0: Arg,
    a1: Arg,
    a2: Arg,
    a3: Arg,
    a4: Arg,
}
#[repr(C)]
#[derive(Copy, Clone)]
struct Args6 {
    a0: Arg,
    a1: Arg,
    a2: Arg,
    a3: Arg,
    a4: Arg,
    a5: Arg,
}

#[derive(Copy, Clone)]
struct SourceLocation {
    module_path: &'static str,
    file: &'static str,
    line: u32,
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
    fn file_name(&self) -> &'static str {
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

        // if let Some(index) = self.file.rfind(std::path::MAIN_SEPARATOR) {
        //     &self.file[index + 1..]
        // } else {
        //     self.file
        // }
    }
}

macro_rules! source_location {
    () => {
        $crate::SourceLocation::__new(module_path!(), file!(), line!())
    };
}

// =============================
// Emit macros 0..6 (compile-time fixed arity)
// (优化#3：SITE const &'static str 写入 entry)
// =============================
// macro_rules! __site {
//     () => {
//         concat!(module_path!(), "::", file!(), "#", line!())
//     };
// }

macro_rules! __emit0 {
    ($logger:expr, $lvl:expr, $fmt:literal) => {{
        #[inline(never)]
        fn __hft_shim(out: &mut dyn std::io::Write, _p: *const u8) {
            let _ = write!(out, $fmt);
        }
        let args = Args0;
        let e = LogEntry::<ENTRY_BYTES>::from_args($lvl, source_location!(), __hft_shim, &args);
        $logger.push(e);
    }};
}

macro_rules! __emit1 {
    ($logger:expr, $lvl:expr, $fmt:literal, $a0:expr) => {{
        #[inline(never)]
        fn __hft_shim(out: &mut dyn std::io::Write, p: *const u8) {
            let a = unsafe { &*(p as *const Args1) };
            let _ = write!(out, $fmt, a.a0);
        }
        let args = Args1 {
            a0: IntoArg::into_arg($a0),
        };
        let e = LogEntry::<ENTRY_BYTES>::from_args($lvl, source_location!(), __hft_shim, &args);
        $logger.push(e);
    }};
}

macro_rules! __emit2 {
    ($logger:expr, $lvl:expr, $fmt:literal, $a0:expr, $a1:expr) => {{
        #[inline(never)]
        fn __hft_shim(out: &mut dyn std::io::Write, p: *const u8) {
            let a = unsafe { &*(p as *const Args2) };
            let _ = write!(out, $fmt, a.a0, a.a1);
        }
        let args = Args2 {
            a0: IntoArg::into_arg($a0),
            a1: IntoArg::into_arg($a1),
        };
        let e = LogEntry::<ENTRY_BYTES>::from_args($lvl, source_location!(), __hft_shim, &args);
        $logger.push(e);
    }};
}

macro_rules! __emit3 {
    ($logger:expr, $lvl:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr) => {{
        #[inline(never)]
        fn __hft_shim(out: &mut dyn std::io::Write, p: *const u8) {
            let a = unsafe { &*(p as *const Args3) };
            let _ = write!(out, $fmt, a.a0, a.a1, a.a2);
        }
        let args = Args3 {
            a0: IntoArg::into_arg($a0),
            a1: IntoArg::into_arg($a1),
            a2: IntoArg::into_arg($a2),
        };
        let e = LogEntry::<ENTRY_BYTES>::from_args($lvl, source_location!(), __hft_shim, &args);
        $logger.push(e);
    }};
}

macro_rules! __emit4 {
    ($logger:expr, $lvl:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr) => {{
        #[inline(never)]
        fn __hft_shim(out: &mut dyn std::io::Write, p: *const u8) {
            let a = unsafe { &*(p as *const Args4) };
            let _ = write!(out, $fmt, a.a0, a.a1, a.a2, a.a3);
        }
        let args = Args4 {
            a0: IntoArg::into_arg($a0),
            a1: IntoArg::into_arg($a1),
            a2: IntoArg::into_arg($a2),
            a3: IntoArg::into_arg($a3),
        };
        let e = LogEntry::<ENTRY_BYTES>::from_args($lvl, source_location!(), __hft_shim, &args);
        $logger.push(e);
    }};
}

macro_rules! __emit5 {
    ($logger:expr, $lvl:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr) => {{
        #[inline(never)]
        fn __hft_shim(out: &mut dyn std::io::Write, p: *const u8) {
            let a = unsafe { &*(p as *const Args5) };
            let _ = write!(out, $fmt, a.a0, a.a1, a.a2, a.a3, a.a4);
        }
        let args = Args5 {
            a0: IntoArg::into_arg($a0),
            a1: IntoArg::into_arg($a1),
            a2: IntoArg::into_arg($a2),
            a3: IntoArg::into_arg($a3),
            a4: IntoArg::into_arg($a4),
        };
        let e = LogEntry::<ENTRY_BYTES>::from_args($lvl, source_location!(), __hft_shim, &args);
        $logger.push(e);
    }};
}

macro_rules! __emit6 {
    ($logger:expr, $lvl:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr) => {{
        #[inline(never)]
        fn __hft_shim(out: &mut dyn std::io::Write, p: *const u8) {
            let a = unsafe { &*(p as *const Args6) };
            let _ = write!(out, $fmt, a.a0, a.a1, a.a2, a.a3, a.a4, a.a5);
        }
        let args = Args6 {
            a0: IntoArg::into_arg($a0),
            a1: IntoArg::into_arg($a1),
            a2: IntoArg::into_arg($a2),
            a3: IntoArg::into_arg($a3),
            a4: IntoArg::into_arg($a4),
            a5: IntoArg::into_arg($a5),
        };
        let e = LogEntry::<ENTRY_BYTES>::from_args($lvl, source_location!(), __hft_shim, &args);
        $logger.push(e);
    }};
}

// =============================
// User macros: Info / Debug / Warn / Error  (0..=6 args)
// =============================
macro_rules! hft_info {
    ($logger:expr, $fmt:literal $(,)?) => {{
        if enabled(Level::Info) { __emit0!($logger, Level::Info, $fmt); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr $(,)?) => {{
        if enabled(Level::Info) { __emit1!($logger, Level::Info, $fmt, $a0); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr $(,)?) => {{
        if enabled(Level::Info) { __emit2!($logger, Level::Info, $fmt, $a0, $a1); }
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

macro_rules! hft_debug {
    ($logger:expr, $fmt:literal $(,)?) => {{
        if enabled(Level::Debug) { __emit0!($logger, Level::Debug, $fmt); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr $(,)?) => {{
        if enabled(Level::Debug) { __emit1!($logger, Level::Debug, $fmt, $a0); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr $(,)?) => {{
        if enabled(Level::Debug) { __emit2!($logger, Level::Debug, $fmt, $a0, $a1); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr $(,)?) => {{
        if enabled(Level::Debug) { __emit3!($logger, Level::Debug, $fmt, $a0, $a1, $a2); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr $(,)?) => {{
        if enabled(Level::Debug) { __emit4!($logger, Level::Debug, $fmt, $a0, $a1, $a2, $a3); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr $(,)?) => {{
        if enabled(Level::Debug) { __emit5!($logger, Level::Debug, $fmt, $a0, $a1, $a2, $a3, $a4); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr $(,)?) => {{
        if enabled(Level::Debug) { __emit6!($logger, Level::Debug, $fmt, $a0, $a1, $a2, $a3, $a4, $a5); }
    }};
}

macro_rules! hft_warn {
    ($logger:expr, $fmt:literal $(,)?) => {{
        if enabled(Level::Warn) { __emit0!($logger, Level::Warn, $fmt); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr $(,)?) => {{
        if enabled(Level::Warn) { __emit1!($logger, Level::Warn, $fmt, $a0); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr $(,)?) => {{
        if enabled(Level::Warn) { __emit2!($logger, Level::Warn, $fmt, $a0, $a1); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr $(,)?) => {{
        if enabled(Level::Warn) { __emit3!($logger, Level::Warn, $fmt, $a0, $a1, $a2); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr $(,)?) => {{
        if enabled(Level::Warn) { __emit4!($logger, Level::Warn, $fmt, $a0, $a1, $a2, $a3); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr $(,)?) => {{
        if enabled(Level::Warn) { __emit5!($logger, Level::Warn, $fmt, $a0, $a1, $a2, $a3, $a4); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr $(,)?) => {{
        if enabled(Level::Warn) { __emit6!($logger, Level::Warn, $fmt, $a0, $a1, $a2, $a3, $a4, $a5); }
    }};
}

macro_rules! hft_error {
    ($logger:expr, $fmt:literal $(,)?) => {{
        if enabled(Level::Error) { __emit0!($logger, Level::Error, $fmt); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr $(,)?) => {{
        if enabled(Level::Error) { __emit1!($logger, Level::Error, $fmt, $a0); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr $(,)?) => {{
        if enabled(Level::Error) { __emit2!($logger, Level::Error, $fmt, $a0, $a1); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr $(,)?) => {{
        if enabled(Level::Error) { __emit3!($logger, Level::Error, $fmt, $a0, $a1, $a2); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr $(,)?) => {{
        if enabled(Level::Error) { __emit4!($logger, Level::Error, $fmt, $a0, $a1, $a2, $a3); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr $(,)?) => {{
        if enabled(Level::Error) { __emit5!($logger, Level::Error, $fmt, $a0, $a1, $a2, $a3, $a4); }
    }};
    ($logger:expr, $fmt:literal, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr $(,)?) => {{
        if enabled(Level::Error) { __emit6!($logger, Level::Error, $fmt, $a0, $a1, $a2, $a3, $a4, $a5); }
    }};
}

// =============================
// Demo
// =============================
#[derive(Debug)]
struct BigObj {
    a: u64,
    b: [u8; 128],
}

fn main() {
    let logger = init_logger(1024);

    println!("size-of-arg={}", size_of::<Arg>());

    if true {
        return;
    }

    let x: i64 = 123;
    let s = "a-very-very-very-very-very-very-very-very-long-string-should-trunc";
    let big = BigObj { a: 42, b: [7u8; 128] };

    hft_info!(logger, "hello x={}", x);
    hft_info!(logger, "mix x={} s={}", x, s);
    hft_debug!(logger, "big={:?}", ptr(&big));

    let lg2 = logger.clone();
    std::thread::spawn(move || {
        for i in 0..200u64 {
            hft_info!(lg2, "[exec] sym={} i={}", "BERAUSDT", i);
        }
    });

    for i in 0..200u64 {
        hft_info!(logger, "[recv] ao={} td-recv-us={}", "GBAMB9uZfZZgZf", i as i64);
        if i % 23 == 0 {
            hft_warn!(logger, "critical [recv] ao={} td-recv-us={}", "GBAMB9uZfZZgZf", i as i64);
        }
    }

    std::thread::sleep(Duration::from_millis(200));
}
