use super::tee;
use std::io::{Cursor, Write};

/// A sink that refuses every write.
struct Closed;
impl Write for Closed {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Every byte reaches both the log the smoke checks collect and the original
/// stderr, in order, across more than one read.
#[test]
fn stderr_reaches_the_log_and_the_original() {
    let text = "smoke observation dropped: full\n".repeat(200);
    let (mut log, mut original) = (Vec::new(), Vec::new());
    tee(Cursor::new(text.clone()), &mut log, &mut original);
    assert_eq!(String::from_utf8_lossy(&log), text);
    assert_eq!(String::from_utf8_lossy(&original), text);
}

/// A failing original stderr does not stop the log, so a drop stays visible.
#[test]
fn a_closed_original_does_not_stop_the_log() {
    let mut log = Vec::new();
    tee(
        Cursor::new("smoke observation dropped: full\n"),
        &mut log,
        Closed,
    );
    assert_eq!(log, b"smoke observation dropped: full\n");
}
