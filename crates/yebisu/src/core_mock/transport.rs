//! Deterministic transport stops for tests; never used in the application.
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

pub(crate) struct Latch {
    pub armed: AtomicBool,
    pub reached: AtomicBool,
    release: mpsc::Sender<()>,
    resume: Mutex<mpsc::Receiver<()>>,
}
impl Default for Latch {
    fn default() -> Self {
        let (release, resume) = mpsc::channel();
        Self {
            armed: AtomicBool::new(false),
            reached: AtomicBool::new(false),
            release,
            resume: Mutex::new(resume),
        }
    }
}
impl Latch {
    pub fn arm(&self) {
        self.reached.store(false, Ordering::Release);
        self.armed.store(true, Ordering::Release);
    }
    pub fn resume(&self) {
        let _ = self.release.send(());
    }
    pub fn pass(&self) {
        if self.armed.swap(false, Ordering::AcqRel) {
            self.reached.store(true, Ordering::Release);
            self.resume
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .recv_timeout(Duration::from_secs(15))
                .unwrap_or_else(|e| panic!("latch: {e}"));
        }
    }
}
#[derive(Clone, Default)]
pub(crate) struct Control {
    pub read: Arc<Latch>,
    pub after_read: Arc<Latch>,
    pub writing: Arc<Latch>,
    pub write_chunk: Arc<AtomicUsize>,
    /// 1: partial byte, 2: whole final write.
    pub write_mode: Arc<AtomicUsize>,
}
pub(crate) struct Guarded {
    socket: UnixStream,
    control: Control,
}
impl Guarded {
    pub fn new(socket: UnixStream, control: Control) -> Self {
        Self { socket, control }
    }
}
impl Read for Guarded {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.control.read.pass();
        let read = self.socket.read(bytes);
        if matches!(read, Ok(n) if n > 0) {
            self.control.after_read.pass();
        }
        read
    }
}
impl Write for Guarded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self.control.write_mode.swap(0, Ordering::AcqRel) {
            1 => {
                let n = self.socket.write(&bytes[..1])?;
                self.control.writing.pass();
                Ok(n)
            }
            2 => {
                self.socket.write_all(bytes)?;
                self.control.writing.pass();
                Ok(bytes.len())
            }
            _ => {
                let chunk = self.control.write_chunk.load(Ordering::Acquire);
                self.socket.write(if chunk == 0 {
                    bytes
                } else {
                    &bytes[..bytes.len().min(chunk)]
                })
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}
