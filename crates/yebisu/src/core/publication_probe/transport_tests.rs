//! Host-shaped observer validation only, not F19 product acceptance.
use super::{
    Control, Journal, Key, WAIT,
    transport::{Observed, ShortWrites},
};
use crate::{terminal::InputTarget, terminal_input::Destination};
use serde_json::json;
use sheprd_core_client::{
    Method,
    frame::{HEADER, Kind, MAX_PAYLOAD, encode},
};
use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Sink {
    bytes: Arc<Mutex<Vec<u8>>>,
    steps: VecDeque<usize>,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let n = self
            .steps
            .pop_front()
            .unwrap_or(bytes.len())
            .min(bytes.len());
        self.bytes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(&bytes[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn hello() -> Vec<u8> {
    format!("{}\n", json!({"jsonrpc":"2.0","id":0,"method":Method::Hello.as_str(),"params":{"framed":true,"generation":1}})).into_bytes()
}
fn frame(channel: u32, kind: Kind, bytes: &[u8]) -> Vec<u8> {
    encode(channel, kind, bytes).unwrap_or_else(|e| panic!("{e}"))
}
fn key(journal: &Journal) -> Key {
    journal.bind(
        Destination {
            generation: 7,
            epoch: 3,
            boot: "fixture".into(),
            target: InputTarget::Pane("core-0".into()),
        },
        11,
    )
}
#[test]
fn host_write_all_interrupted_short_header_payload_and_multiple_frames() -> io::Result<()> {
    let journal = Journal::new("cursor-positive");
    let sink = Sink::default();
    let bytes = sink.bytes.clone();
    let mut writer = Observed::new(
        ShortWrites {
            inner: sink,
            calls: 0,
        },
        journal.clone(),
        Control::default(),
    );
    let inputs = [
        hello(),
        frame(0, Kind::Data, br#"{"jsonrpc":"2.0"}"#),
        frame(1, Kind::Data, b"first semantic text"),
        frame(1, Kind::Data, b"second semantic text"),
        frame(1, Kind::Credit, &5_u32.to_le_bytes()),
    ];
    for input in &inputs {
        writer.write_all(input)?;
    }
    writer.flush()?;
    drop(writer);
    assert_eq!(
        *bytes.lock().unwrap_or_else(|e| e.into_inner()),
        inputs.concat()
    );
    let rows = journal.rows();
    assert!(rows.iter().any(|r| r["kind"] == "writer-interrupted"));
    assert_eq!(
        rows.iter()
            .filter(|r| r["kind"] == "writer-frame-complete")
            .count(),
        inputs.len()
    );
    assert!(
        rows.iter()
            .any(|r| r["kind"] == "writer-accepted" && r["data"]["accepted_offsets"][1] == 1)
    );
    Ok(())
}
#[test]
fn payload_suffix_is_not_parsed_as_a_new_header() -> io::Result<()> {
    let journal = Journal::new("payload-suffix");
    let sink = Sink {
        steps: VecDeque::from([usize::MAX, HEADER + 2, usize::MAX]),
        ..Sink::default()
    };
    let bytes = sink.bytes.clone();
    let mut writer = Observed::new(sink, journal.clone(), Control::default());
    let greeting = hello();
    writer.write_all(&greeting)?;
    let message = frame(
        1,
        Kind::Data,
        b"payload with arbitrary header-looking suffix",
    );
    assert_eq!(writer.write(&message)?, HEADER + 2);
    writer.write_all(&message[HEADER + 2..])?;
    writer.flush()?;
    drop(writer);
    assert_eq!(
        *bytes.lock().unwrap_or_else(|e| e.into_inner()),
        [greeting, message].concat()
    );
    assert_eq!(
        journal
            .rows()
            .iter()
            .filter(|r| r["kind"] == "writer-frame-complete")
            .count(),
        2
    );
    Ok(())
}
#[test]
fn zero_write_is_an_error_not_a_clean_empty_transcript() {
    let mut writer = Observed::new(
        Sink {
            steps: VecDeque::from([0]),
            ..Sink::default()
        },
        Journal::new("zero"),
        Control::default(),
    );
    assert_eq!(
        writer.write_all(&hello()).err().map(|e| e.kind()),
        Some(io::ErrorKind::WriteZero)
    );
}
#[test]
fn changed_short_header_suffix_fails_setup() -> io::Result<()> {
    let mut writer = Observed::new(
        Sink {
            steps: VecDeque::from([usize::MAX, 2]),
            ..Sink::default()
        },
        Journal::new("changed-suffix"),
        Control::default(),
    );
    writer.write_all(&hello())?;
    let message = frame(1, Kind::Data, b"text");
    assert_eq!(writer.write(&message)?, 2);
    assert!(writer.write(&message[3..]).is_err());
    Ok(())
}
#[test]
fn truncated_unknown_oversized_or_coalesced_initial_frames_fail_setup() -> io::Result<()> {
    let valid = frame(1, Kind::Data, b"text");
    let mut unknown = valid.clone();
    unknown[4] = 255;
    let mut huge = valid[..HEADER].to_vec();
    huge[5..HEADER].copy_from_slice(&((MAX_PAYLOAD + 1) as u32).to_le_bytes());
    for invalid in [
        valid[..2].to_vec(),
        valid[..HEADER + 1].to_vec(),
        unknown,
        huge,
        [valid.clone(), valid].concat(),
    ] {
        let mut writer = Observed::new(
            Sink::default(),
            Journal::new("invalid-shape"),
            Control::default(),
        );
        writer.write_all(&hello())?;
        assert!(writer.write_all(&invalid).is_err());
    }
    Ok(())
}
#[test]
fn b0_excludes_rpc_and_controls_and_rejects_wrong_terminal_frame() -> io::Result<()> {
    let journal = Journal::new("wrong-frame");
    let control = Control::default();
    let mut writer = Observed::new(Sink::default(), journal.clone(), control.clone());
    let (_arrived, _release) = control.arm(key(&journal), b"selected");
    writer.write_all(&hello())?;
    writer.write_all(&frame(0, Kind::Data, b"selected"))?;
    writer.write_all(&frame(1, Kind::Resize, &[80, 0, 24, 0]))?;
    assert!(
        writer
            .write_all(&frame(2, Kind::Data, b"selected"))
            .is_err()
    );
    Ok(())
}
#[test]
fn b0_wrong_epoch_release_rejects_before_any_selected_byte() {
    let journal = Journal::new("wrong-release");
    let control = Control::default();
    let bound = key(&journal);
    let (arrived, release) = control.arm(bound.clone(), b"selected");
    let sink = Sink::default();
    let bytes = sink.bytes.clone();
    let worker = std::thread::spawn(move || {
        let mut writer = Observed::new(sink, journal, control);
        writer.write_all(&hello()).unwrap_or_else(|e| panic!("{e}"));
        writer.write_all(&frame(1, Kind::Data, b"selected"))
    });
    assert_eq!(
        arrived.recv_timeout(WAIT).unwrap_or_else(|e| panic!("{e}")),
        bound
    );
    assert_eq!(*bytes.lock().unwrap_or_else(|e| e.into_inner()), hello());
    let mut wrong = bound;
    wrong.input_epoch += 1;
    release.send(wrong).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        worker
            .join()
            .unwrap_or_else(|_| panic!("worker panic"))
            .is_err()
    );
    assert_eq!(*bytes.lock().unwrap_or_else(|e| e.into_inner()), hello());
}
