//! Capture actual fd 1/2 for every QA process, before diagnostics/provider startup.
use std::{
    fs::OpenOptions,
    io::{self, Read},
    os::fd::AsRawFd,
    path::PathBuf,
};
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
    let binding = std::env::var("YEBISU_P11_BINDING").map_err(|_| invalid_image())?;
    if binding.len() != 64 || !binding.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid_image());
    }
    let mut image = Vec::new();
    std::fs::File::open(std::env::current_exe()?)?
        .take(1024 * 1024)
        .read_to_end(&mut image)?;
    let image_uuid = macho_uuid(&image)?;
    // Binding is a runner correlation label; UUID comes from this process's
    // actual executable file and is independently compared with installed bytes.
    eprintln!(
        "p11-native-capture pid={} binding={binding} image_uuid={image_uuid}",
        std::process::id()
    );
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

fn invalid_image() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid QA image identity")
}
fn word(bytes: &[u8], offset: usize) -> io::Result<u32> {
    let data = bytes.get(offset..offset + 4).ok_or_else(invalid_image)?;
    Ok(u32::from_le_bytes(
        data.try_into().map_err(|_| invalid_image())?,
    ))
}
// The collector builds one aarch64-apple-ios-sim slice, never a fat image.
fn macho_uuid(bytes: &[u8]) -> io::Result<String> {
    if word(bytes, 0)? != 0xfeed_facf || word(bytes, 4)? != 0x0100_000c {
        return Err(invalid_image());
    }
    let count = word(bytes, 16)? as usize;
    let end = 32usize
        .checked_add(word(bytes, 20)? as usize)
        .ok_or_else(invalid_image)?;
    if end > bytes.len() || count > (end - 32) / 8 {
        return Err(invalid_image());
    }
    let mut offset = 32;
    let mut found = None;
    for _ in 0..count {
        let command = word(bytes, offset)?;
        let size = word(bytes, offset + 4)? as usize;
        if size < 8 || size > end - offset {
            return Err(invalid_image());
        }
        if command == 0x1b {
            if size != 24 || found.is_some() {
                return Err(invalid_image());
            }
            let hex: String = bytes[offset + 8..offset + 24]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            found = Some(format!(
                "{}-{}-{}-{}-{}",
                &hex[..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..]
            ));
        }
        offset += size;
    }
    if offset != end {
        return Err(invalid_image());
    }
    found.ok_or_else(invalid_image)
}
