//! QA builds copy the app's stderr into `Documents/app-stderr.log`, so the
//! smoke checks can put it in their artifacts. An app that XCTest launches
//! has no stderr anyone keeps, so a `smoke observation dropped` line or an
//! observation writer error was lost (hosted p10, run 44406830068 job
//! 135164241872). Simulator builds with a QA observation variable only; the
//! original stderr still gets every byte.
use std::io::{ErrorKind, Read, Write};

/// The file the smoke checks collect, under the app's `Documents`.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) const LOG: &str = "app-stderr.log";

/// Copies everything `source` yields to `log` and to `original`. A sink that
/// fails is skipped from then on and the other keeps receiving. It returns
/// only at the end of `source`: in the app the source is a pipe whose write
/// end is fd 2, so it never ends and stderr never meets a closed pipe.
pub(crate) fn tee(mut source: impl Read, mut log: impl Write, mut original: impl Write) {
    let mut buffer = [0_u8; 4096];
    let (mut log_open, mut original_open) = (true, true);
    loop {
        let read = match source.read(&mut buffer) {
            Ok(0) => return,
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
        };
        let bytes = &buffer[..read];
        log_open = log_open && log.write_all(bytes).and_then(|()| log.flush()).is_ok();
        original_open = original_open && original.write_all(bytes).is_ok();
    }
}

/// Starts the copy when a QA observation variable is set and the P11
/// capture (which owns fd 1 and 2 itself) is not.
#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) fn start() -> std::io::Result<()> {
    let observed = ["YEBISU_SMOKE_OBSERVE", "YEBISU_P2_OBSERVE"]
        .iter()
        .any(|name| std::env::var_os(name).is_some());
    if !observed || std::env::var_os("YEBISU_P11_CAPTURE").is_some() {
        return Ok(());
    }
    let home = std::env::var_os("HOME").ok_or(ErrorKind::NotFound)?;
    let directory = std::path::PathBuf::from(home).join("Documents");
    std::fs::create_dir_all(&directory)?;
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join(LOG))?;
    log.write_all(format!("app-stderr launch pid={}\n", std::process::id()).as_bytes())?;
    let (source, original) = redirect()?;
    std::thread::Builder::new()
        .name("yebisu-qa-stderr".into())
        .spawn(move || tee(source, log, original))?;
    Ok(())
}

#[cfg(all(target_os = "ios", target_abi = "sim"))]
#[allow(
    unsafe_code,
    reason = "Simulator-only QA points fd 2 at a pipe it reads, as p11_observer::capture does"
)]
fn redirect() -> std::io::Result<(std::fs::File, std::fs::File)> {
    use std::os::fd::{AsRawFd, FromRawFd};
    unsafe extern "C" {
        fn pipe(fds: *mut std::ffi::c_int) -> std::ffi::c_int;
        fn dup(fd: std::ffi::c_int) -> std::ffi::c_int;
        fn dup2(from: std::ffi::c_int, to: std::ffi::c_int) -> std::ffi::c_int;
    }
    const STDERR: std::ffi::c_int = 2;
    let mut fds = [0 as std::ffi::c_int; 2];
    // SAFETY: `fds` has room for the two descriptors pipe(2) writes.
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: pipe(2) just returned these two descriptors; nothing else owns them.
    let (source, sink) = unsafe {
        (
            std::fs::File::from_raw_fd(fds[0]),
            std::fs::File::from_raw_fd(fds[1]),
        )
    };
    // SAFETY: fd 2 is open for the whole process; dup(2) returns a new descriptor.
    let original = unsafe { dup(STDERR) };
    if original < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: dup(2) just returned this descriptor; nothing else owns it.
    let original = unsafe { std::fs::File::from_raw_fd(original) };
    // SAFETY: both descriptors are open; dup2(2) makes fd 2 a second write end
    // of the pipe, so dropping `sink` below closes only the first one.
    if unsafe { dup2(sink.as_raw_fd(), STDERR) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    drop(sink);
    Ok((source, original))
}

#[cfg(test)]
#[path = "qa_stderr_tests.rs"]
mod tests;
