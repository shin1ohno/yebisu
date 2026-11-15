//! Pinned SDK host shape: one complete frame passed to write_all, then exact suffixes.
//! B0 holds before the FIRST underlying write; no filtering/cancellation/replay.
use super::{Journal, Key, TOTAL, WAIT};
use serde_json::json;
use sheprd_core_client::{
    Method,
    frame::{HEADER, Kind, MAX_PAYLOAD},
};
use std::{
    io::{self, Read, Write},
    sync::{Arc, Mutex, mpsc},
    time::Instant,
};

struct Arm {
    key: Key,
    frame: Vec<u8>,
    arrived: mpsc::SyncSender<Key>,
    resume: mpsc::Receiver<Key>,
}
#[derive(Clone, Default)]
pub(crate) struct Control(Arc<Mutex<Option<Arm>>>);
impl Control {
    pub fn arm(&self, key: Key, payload: &[u8]) -> (mpsc::Receiver<Key>, mpsc::SyncSender<Key>) {
        assert_ne!(key.channel, 0);
        let frame = sheprd_core_client::frame::encode(key.channel, Kind::Data, payload)
            .unwrap_or_else(|e| panic!("fixture frame: {e}"));
        let (arrived, observed) = mpsc::sync_channel(1);
        let (release, resume) = mpsc::sync_channel(1);
        let mut slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        assert!(slot.is_none(), "only one B0 arm per fixture");
        *slot = Some(Arm {
            key,
            frame,
            arrived,
            resume,
        });
        (observed, release)
    }
}
struct Cursor {
    bytes: Vec<u8>,
    accepted: usize,
    sequence: u64,
    channel: Option<u32>,
}
pub(super) struct Observed<S> {
    inner: S,
    journal: Journal,
    control: Control,
    hello: bool,
    cursor: Option<Cursor>,
    frames: u64,
    deadline: Instant,
    failed: bool,
}
impl<S> Observed<S> {
    pub fn new(inner: S, journal: Journal, control: Control) -> Self {
        Self {
            inner,
            journal,
            control,
            hello: true,
            cursor: None,
            frames: 0,
            deadline: Instant::now() + TOTAL,
            failed: false,
        }
    }
    fn begin(&mut self, bytes: &[u8]) -> io::Result<()> {
        let channel;
        if self.hello {
            if bytes.len() > 65536
                || !bytes.ends_with(b"\n")
                || bytes[..bytes.len() - 1].contains(&b'\n')
            {
                return Err(io::Error::other("ambiguous initial plain hello"));
            }
            let message: serde_json::Value =
                serde_json::from_slice(bytes).map_err(io::Error::other)?;
            if message["method"] != Method::Hello.as_str() || message["params"]["framed"] != true {
                return Err(io::Error::other("not the actual plain framed hello"));
            }
            channel = None;
        } else {
            let header = bytes
                .get(..HEADER)
                .ok_or_else(|| io::Error::other("initial frame header incomplete"))?;
            let ch = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
            let kind = Kind::try_from(header[4]).map_err(io::Error::other)?;
            let length = u32::from_le_bytes([header[5], header[6], header[7], header[8]]) as usize;
            if length > MAX_PAYLOAD || bytes.len() != HEADER + length {
                return Err(io::Error::other("initial frame length/shape ambiguous"));
            }
            channel = Some(ch);
            // RPC channel0 Data and non-Data controls must never trigger B0.
            if ch != 0 && kind == Kind::Data {
                let arm = self
                    .control
                    .0
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
                if let Some(arm) = arm {
                    if ch != arm.key.channel || bytes != arm.frame.as_slice() {
                        return Err(io::Error::other("B0 semantic frame identity mismatch"));
                    }
                    // Epoch is the separately armed producer identity, NOT stamped
                    // onto the SDK frame from a current epoch at dequeue.
                    self.journal
                        .record("dequeue-before-write-arrived", arm.key.json())?;
                    arm.arrived
                        .send(arm.key.clone())
                        .map_err(io::Error::other)?;
                    if arm.resume.recv_timeout(WAIT).map_err(io::Error::other)? != arm.key {
                        return Err(io::Error::other("B0 release key mismatch"));
                    }
                    self.journal
                        .record("dequeue-before-write-release", arm.key.json())?;
                }
            }
        }
        self.frames = self
            .frames
            .checked_add(1)
            .ok_or_else(|| io::Error::other("frame sequence overflow"))?;
        self.journal.record("writer-admission", json!({"writer_sequence":self.frames,"channel":channel,"bytes":bytes.len(),"plain_hello":self.hello}))?;
        self.cursor = Some(Cursor {
            bytes: bytes.to_vec(),
            accepted: 0,
            sequence: self.frames,
            channel,
        });
        Ok(())
    }
}
impl<S: Read> Read for Observed<S> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.inner.read(bytes)
    }
}
impl<S: Write> Observed<S> {
    fn step(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if Instant::now() >= self.deadline {
            return Err(io::Error::other("probe total deadline"));
        }
        if bytes.is_empty() {
            return Err(io::Error::other("ambiguous empty write"));
        }
        if self.cursor.is_none() {
            self.begin(bytes)?;
        }
        let cursor = self
            .cursor
            .as_mut()
            .ok_or_else(|| io::Error::other("missing frame cursor"))?;
        if bytes != &cursor.bytes[cursor.accepted..] {
            return Err(io::Error::other("write_all suffix changed"));
        }
        match self.inner.write(bytes) {
            Ok(0) => Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "probe underlying zero write",
            )),
            Ok(count) => {
                if count > bytes.len() {
                    return Err(io::Error::other("underlying write count exceeded suffix"));
                }
                let before = cursor.accepted;
                cursor.accepted += count;
                self.journal.record("writer-accepted", json!({"writer_sequence":cursor.sequence,"channel":cursor.channel,"accepted_offsets":[before,cursor.accepted]}))?;
                if cursor.accepted == cursor.bytes.len() {
                    self.journal.record("writer-frame-complete", json!({"writer_sequence":cursor.sequence,"channel":cursor.channel,"bytes":cursor.accepted}))?;
                    self.cursor = None;
                    self.hello = false;
                }
                Ok(count)
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                self.journal.record(
                    "writer-interrupted",
                    json!({"writer_sequence":cursor.sequence,"offset":cursor.accepted}),
                )?;
                Err(error)
            }
            Err(error) => Err(error),
        }
    }
}
impl<S: Write> Write for Observed<S> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = self.step(bytes);
        if result
            .as_ref()
            .is_err_and(|e| e.kind() != io::ErrorKind::Interrupted)
        {
            self.failed = true;
            self.journal.fail();
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.cursor.is_some() {
            self.failed = true;
            self.journal.fail();
            return Err(io::Error::other("flush with incomplete frame"));
        }
        let result = self.inner.flush();
        if result.is_err() {
            self.failed = true;
            self.journal.fail();
        }
        result
    }
}
impl<S> Drop for Observed<S> {
    fn drop(&mut self) {
        let complete = !self.failed
            && self.cursor.is_none()
            && !self.hello
            && self
                .control
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none();
        if !complete {
            self.journal.fail();
        }
        let _ = self.journal.record(
            "transport-dropped",
            json!({"complete":complete,"writer_sequence":self.frames}),
        );
    }
}

/// Exercise real write_all retries/short header and payload progress on the socket.
pub(super) struct ShortWrites<S> {
    pub inner: S,
    pub calls: usize,
}
impl<S: Read> Read for ShortWrites<S> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.inner.read(bytes)
    }
}
impl<S: Write> Write for ShortWrites<S> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let step = self.calls % 6;
        self.calls += 1;
        if step == 0 {
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        let cap = match step {
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 5,
            _ => bytes.len(),
        };
        self.inner.write(&bytes[..bytes.len().min(cap)])
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
