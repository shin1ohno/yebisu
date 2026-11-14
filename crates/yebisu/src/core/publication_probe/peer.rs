//! Disposable socketpair only. EOF plus joined peer owns the complete transcript.
use super::{
    Control, Journal, TOTAL, WAIT,
    transport::{Observed, ShortWrites},
};
use serde_json::{Value, json};
use sheprd_core_client::{
    Core, CoreBackend, Method,
    frame::{Decoder, Frame, Kind, MAX_PAYLOAD, encode},
};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    sync::mpsc,
    thread::JoinHandle,
    time::Instant,
};

pub(crate) struct Transcript {
    pub frames: Vec<Frame>,
    pub marker_id: Value,
    pub receiver_eof: bool,
}
pub(crate) struct Peer {
    pub received: mpsc::Receiver<Frame>,
    completed: mpsc::Receiver<io::Result<Transcript>>,
    worker: JoinHandle<()>,
}
impl Peer {
    pub fn connect(journal: Journal, control: Control) -> crate::Result<(CoreBackend, Self)> {
        let (client, server) = UnixStream::pair()?;
        client.set_read_timeout(Some(std::time::Duration::from_millis(10)))?;
        client.set_write_timeout(Some(WAIT))?;
        server.set_read_timeout(Some(std::time::Duration::from_millis(20)))?;
        server.set_write_timeout(Some(WAIT))?;
        let (send, received) = mpsc::sync_channel(16);
        let (done, completed) = mpsc::sync_channel(1);
        let peer_journal = journal.clone();
        let worker = std::thread::spawn(move || {
            let result = serve(server, &peer_journal, send);
            if result.is_err() {
                peer_journal.fail();
            }
            let _ = done.send(result);
        });
        let transport = Observed::new(
            ShortWrites {
                inner: client,
                calls: 0,
            },
            journal,
            control,
        );
        let backend = CoreBackend::new(Core::over(transport)?, 10000);
        Ok((
            backend,
            Self {
                received,
                completed,
                worker,
            },
        ))
    }
    pub fn finish(self) -> Transcript {
        let transcript = self
            .completed
            .recv_timeout(WAIT)
            .unwrap_or_else(|e| panic!("peer tail/EOF missing: {e}"))
            .unwrap_or_else(|e| panic!("peer transcript invalid: {e}"));
        self.worker
            .join()
            .unwrap_or_else(|_| panic!("peer panicked"));
        transcript
    }
}
fn terminal() -> Value {
    json!({"address":"t1","labels":{},"argv":["fixture"],"cwd":"/fixture",
           "state":"live","created_ms":1,"screen":"","cols":80,"rows":24})
}
fn serve(
    mut server: UnixStream,
    journal: &Journal,
    receipts: mpsc::SyncSender<Frame>,
) -> io::Result<Transcript> {
    let deadline = Instant::now() + TOTAL;
    let mut line = Vec::new();
    loop {
        if Instant::now() >= deadline || line.len() >= 65536 {
            return Err(io::Error::other("peer hello bound"));
        }
        let mut byte = [0];
        match server.read(&mut byte) {
            Ok(0) => return Err(io::Error::other("peer EOF before hello")),
            Ok(_) => {
                line.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
    }
    let hello: Value = serde_json::from_slice(&line).map_err(io::Error::other)?;
    if hello["method"] != Method::Hello.as_str() || hello["params"]["framed"] != true {
        return Err(io::Error::other("peer wrong plain hello"));
    }
    journal.record("receiver-hello", json!({"id":hello["id"]}))?;
    let reply = json!({"jsonrpc":"2.0","id":hello["id"],"result":{"generation":1,"version":"0.1.13","framed":true,"features":[],"core":{"id":"c0ffee","name":"private-probe"},"methods":Method::ALL.iter().map(|m|m.as_str()).collect::<Vec<_>>()}});
    server.write_all(format!("{reply}\n").as_bytes())?;
    let mut decoder = Decoder::default();
    let mut buffer = [0; 8192];
    let mut transcript = Transcript {
        frames: Vec::new(),
        marker_id: Value::Null,
        receiver_eof: false,
    };
    let mut attached = false;
    let mut spawned = false;
    let mut terminal_bytes = 0;
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::other("peer total deadline"));
        }
        if let Some(frame) = decoder.next_frame().map_err(io::Error::other)? {
            if transcript.frames.len() >= 128 {
                return Err(io::Error::other("peer frame bound"));
            }
            let sequence = transcript.frames.len() + 1;
            journal.record("receiver-frame", json!({"receiver_sequence":sequence,"channel":frame.channel,"kind":u8::from(frame.kind),"bytes":frame.payload.len()}))?;
            transcript.frames.push(frame.clone());
            if frame.channel != 0 {
                if frame.channel != 1 || !attached {
                    return Err(io::Error::other("peer unexpected channel"));
                }
                if frame.kind == Kind::Data {
                    terminal_bytes += frame.payload.len();
                    if terminal_bytes > 65536 {
                        return Err(io::Error::other("peer fixture payload bound"));
                    }
                    receipts.try_send(frame).map_err(io::Error::other)?;
                }
                continue;
            }
            if frame.kind != Kind::Data {
                return Err(io::Error::other("peer unexpected channel0 kind"));
            }
            let request: Value =
                serde_json::from_slice(&frame.payload).map_err(io::Error::other)?;
            let mut initial_frame = false;
            let result = match request["method"].as_str() {
                Some("terminal.list") => {
                    if request["params"]["all"] == true {
                        if !transcript.marker_id.is_null() {
                            return Err(io::Error::other("duplicate settlement marker"));
                        }
                        transcript.marker_id = request["id"].clone();
                        journal.record(
                            "receiver-ordered-list",
                            json!({"rpc_id":request["id"],"receiver_sequence":sequence}),
                        )?;
                    }
                    json!({"terminals":if spawned { vec![terminal()] } else { vec![] }})
                }
                Some("terminal.run") => {
                    if spawned {
                        return Err(io::Error::other("unexpected second spawn"));
                    }
                    spawned = true;
                    terminal()
                }
                Some("terminal.attach") => {
                    if !spawned || attached || request["params"]["terminal"] != "t1" {
                        return Err(io::Error::other("ambiguous attachment"));
                    }
                    attached = true;
                    initial_frame = true;
                    json!({"channel":1,"terminal":terminal()})
                }
                _ => return Err(io::Error::other("peer unexpected RPC")),
            };
            let reply = json!({"jsonrpc":"2.0","id":request["id"],"result":result});
            server.write_all(
                &encode(0, Kind::Data, reply.to_string().as_bytes()).map_err(io::Error::other)?,
            )?;
            if initial_frame {
                server.write_all(
                    &encode(1, Kind::Data, b"\x1b[2J\x1b[Hfixture ready\r\n$ ")
                        .map_err(io::Error::other)?,
                )?;
            }
            continue;
        }
        if decoder.pending() > MAX_PAYLOAD + 9 {
            return Err(io::Error::other("peer undecoded buffer bound"));
        }
        match server.read(&mut buffer) {
            Ok(0) => {
                if decoder.pending() != 0 || transcript.marker_id.is_null() {
                    return Err(io::Error::other("peer unsettled/truncated EOF"));
                }
                transcript.receiver_eof = true;
                journal.record(
                    "receiver-eof",
                    json!({"frames":transcript.frames.len(),"marker_id":transcript.marker_id}),
                )?;
                return Ok(transcript);
            }
            Ok(count) => decoder.push(&buffer[..count]),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
    }
}
