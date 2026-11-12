//! Capture actual fd 1/2 for every QA process, before diagnostics/provider startup.
use std::{fs::OpenOptions, io, os::fd::AsRawFd, path::PathBuf};
pub(crate) fn start() -> io::Result<()> {
    if std::env::var_os("YEBISU_P11_CAPTURE").is_none() {
        return Ok(());
    }
    let home = std::env::var_os("HOME").ok_or(io::ErrorKind::NotFound)?;
    let directory = PathBuf::from(home).join("Documents/p11-native");
    std::fs::create_dir_all(&directory)?;
    // create_new rejects PID reuse instead of mixing two launches silently.
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(format!("launch-{}.log", std::process::id())))?;
    redirect(file.as_raw_fd())?;
    eprintln!("p11-native-capture pid={}", std::process::id());
    Ok(())
}
#[allow(
    unsafe_code,
    reason = "Simulator-only QA captures real process stdout/stderr file descriptors"
)]
fn redirect(fd: std::os::fd::RawFd) -> io::Result<()> {
    // SAFETY: OpenOptions owns a live descriptor. dup2 duplicates it to the
    // standard descriptors; dropping File closes only the original descriptor.
    for target in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        if unsafe { libc::dup2(fd, target) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
