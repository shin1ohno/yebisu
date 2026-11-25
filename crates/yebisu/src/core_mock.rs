//! Private socketpair peer speaking the pinned core framing and hello protocol.
pub(crate) mod doc;
pub(crate) mod paste;
#[cfg(test)]
mod qa_tick_tests;
#[cfg(test)]
pub(crate) mod transport;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sheprd_core_client::{
    Core, CoreBackend, Method,
    frame::{Decoder, Frame, Kind, encode},
};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

/// The remote terminal namespace and documents outlive any one connection.
#[derive(Clone)]
pub(crate) struct Host {
    terminals: Arc<Mutex<Vec<String>>>,
    doc: doc::Store,
}
impl Default for Host {
    fn default() -> Self {
        Self {
            terminals: Arc::new(Mutex::new(vec!["t1".into(), "t2".into()])),
            doc: doc::Store::qa(),
        }
    }
}

/// `YEBISU_CORE_QA` (p9, plan v2 7.14): the listed terminals report raw
/// statuses, so AGENTS shows the Error row and leaves the shells out.
fn qa_statuses() -> bool {
    cfg!(target_abi = "sim") && std::env::var_os("YEBISU_CORE_QA").is_some()
}

/// The QA status of a mock terminal: `t2` reports Error; `t1` and every
/// spawned pane report nothing (status-less shells, Q4).
pub(crate) fn qa_status(id: &str) -> Option<Value> {
    (id == "t2").then(|| json!({"app":"qa","state":"error","title":"QA error agent"}))
}

/// T-IM-S4 (#139): the mock has no shell to run a background job, so under
/// `YEBISU_CORE_QA` the typed line `yebisu-139-tick` makes its channel print
/// one `.` per interval for a while. The output moves the cursor while the
/// Simulator check holds a composition open.
struct QaTicks {
    enabled: bool,
    every: Duration,
    typed: Vec<u8>,
    // (channel, next tick, ticks left)
    due: Option<(u32, std::time::Instant, u32)>,
}
impl QaTicks {
    const LINE: &[u8] = b"yebisu-139-tick\r";
    fn new(enabled: bool, every: Duration) -> Self {
        Self {
            enabled,
            every,
            typed: Vec::new(),
            due: None,
        }
    }
    fn typed(&mut self, channel: u32, bytes: &[u8]) {
        if !self.enabled {
            return;
        }
        // Return may arrive as either byte.
        self.typed
            .extend(bytes.iter().map(|&b| if b == b'\n' { b'\r' } else { b }));
        let stale = self.typed.len().saturating_sub(Self::LINE.len());
        self.typed.drain(..stale);
        if self.typed == Self::LINE {
            self.due = Some((channel, std::time::Instant::now() + self.every, 60));
        }
    }
    fn tick(&mut self) -> Option<u32> {
        let (channel, next, left) = self.due.as_mut()?;
        if std::time::Instant::now() < *next {
            return None;
        }
        let channel = *channel;
        *next += self.every;
        *left -= 1;
        if *left == 0 {
            self.due = None;
        }
        Some(channel)
    }
}

type Receipts = mpsc::Receiver<(u32, Vec<u8>)>;
#[cfg(test)]
pub(crate) fn recorded(report_all: bool) -> crate::Result<(CoreBackend, Receipts)> {
    Host::default().recorded(report_all)
}
impl Host {
    pub(crate) fn recorded(&self, report_all: bool) -> crate::Result<(CoreBackend, Receipts)> {
        self.with_paste(report_all, paste::Probe::default())
    }
    pub(crate) fn with_paste(
        &self,
        report_all: bool,
        probe: paste::Probe,
    ) -> crate::Result<(CoreBackend, Receipts)> {
        #[cfg(test)]
        {
            self.with_transport(report_all, probe, None)
        }
        #[cfg(not(test))]
        self.with_transport(report_all, probe)
    }
    fn with_transport(
        &self,
        report_all: bool,
        probe: paste::Probe,
        #[cfg(test)] gate: Option<transport::Control>,
    ) -> crate::Result<(CoreBackend, Receipts)> {
        let namespace = self.terminals.clone();
        let doc = self.doc.clone();
        let (client, mut server) = UnixStream::pair()?;
        client.set_read_timeout(Some(Duration::from_millis(10)))?;
        client.set_write_timeout(Some(Duration::from_secs(5)))?;
        server.set_read_timeout(Some(Duration::from_millis(10)))?;
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let _closed = paste::Closed(probe.clone());
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                let mut line = Vec::new();
                let mut byte = [0];
                while !line.ends_with(b"\n") {
                    match server.read_exact(&mut byte) {
                        Ok(()) => {}
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) =>
                        {
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    }
                    line.push(byte[0]);
                }
                let hello: Value = serde_json::from_slice(&line)?;
                let reply = json!({"jsonrpc":"2.0","id":hello["id"],"result":{"generation":1,"version":"0.1.13","framed":true,"features":[],"core":{"id":"c0ffee","name":"private-peer"},"methods":Method::ALL.iter().filter(|m| probe.advertise || **m != Method::TerminalPut).map(|m|m.as_str()).collect::<Vec<_>>()}});
                server.write_all(format!("{reply}\n").as_bytes())?;
                let mut decoder = Decoder::default();
                let mut buffer = [0; 8192];
                let mut next_channel = 1;
                let mut channels = std::collections::BTreeMap::new();
                let mut pending = std::collections::BTreeMap::<usize, Value>::new();
                let mut ticks = QaTicks::new(qa_statuses(), Duration::from_secs(2));
                // Event subscriptions never echo input; `seen` counts doc events.
                let (mut subscriptions, mut seen) = (std::collections::BTreeSet::new(), 0);
                loop {
                    let frame: Frame = loop {
                        if probe.close.load(std::sync::atomic::Ordering::Acquire) {
                            return Ok(());
                        }
                        if let Some(channel) = ticks.tick()
                            && channels.contains_key(&channel)
                        {
                            server.write_all(&encode(channel, Kind::Data, b".")?)?;
                        }
                        if probe.end.swap(false, std::sync::atomic::Ordering::AcqRel) {
                            for channel in channels.keys() {
                                server.write_all(&encode(
                                    *channel,
                                    Kind::Close,
                                    br#"{"reason":"exited","exit_code":0}"#,
                                )?)?;
                            }
                        }
                        let released = std::mem::take(&mut probe.lock().releases);
                        for (index, receipt) in released {
                            if let Some(request) = pending.remove(&index) {
                                send_put_reply(&mut server, &request, receipt)?;
                                #[cfg(test)]
                                probe
                                    .replies_written
                                    .fetch_add(1, std::sync::atomic::Ordering::Release);
                            }
                        }
                        for line in doc.events_after(&mut seen) {
                            for channel in &subscriptions {
                                server.write_all(&encode(*channel, Kind::Data, &line)?)?;
                            }
                        }
                        #[cfg(test)]
                        probe.before_read.pass();
                        if let Some(frame) = decoder.next_frame()? {
                            break frame;
                        }
                        let count = match server.read(&mut buffer) {
                            Ok(count) => count,
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                ) =>
                            {
                                continue;
                            }
                            Err(e) => return Err(e.into()),
                        };
                        if count == 0 {
                            probe.eof.store(true, std::sync::atomic::Ordering::Release);
                            return Ok(());
                        }
                        decoder.push(&buffer[..count]);
                    };
                    if frame.channel != 0 {
                        if frame.kind == Kind::Close {
                            subscriptions.remove(&frame.channel);
                        }
                        if frame.kind == Kind::Data {
                            probe.lock().inputs.push((
                                channels.get(&frame.channel).cloned().unwrap_or_default(),
                                frame.payload.clone(),
                            ));
                            #[cfg(all(target_os = "ios", target_abi = "sim"))]
                            if let Some(home) = std::env::var_os("HOME") {
                                let path = std::path::PathBuf::from(home)
                                    .join("Documents/p9-core-wire.jsonl");
                                if let Ok(mut file) = std::fs::OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(path)
                                {
                                    let _ = crate::observation::write_jsonl_line(
                                        &mut file,
                                        &json!({"channel":frame.channel,"terminal":channels.get(&frame.channel),"bytes":frame.payload}),
                                    );
                                }
                            }
                            // qa.state's `peer` (S4): bracketed pastes on the wire.
                            #[cfg(all(target_os = "ios", target_abi = "sim"))]
                            crate::qa_ax::peer::wire(&frame.payload);
                            if channels.contains_key(&frame.channel) {
                                if frame.payload == b"exit\r" {
                                    server.write_all(&encode(
                                        frame.channel,
                                        Kind::Data,
                                        b"\r\npane exited",
                                    )?)?;
                                    server.write_all(&encode(
                                        frame.channel,
                                        Kind::Close,
                                        br#"{"reason":"exited","exit_code":0}"#,
                                    )?)?;
                                    channels.remove(&frame.channel);
                                } else {
                                    server.write_all(&encode(
                                        frame.channel,
                                        Kind::Data,
                                        &frame.payload,
                                    )?)?;
                                    ticks.typed(frame.channel, &frame.payload);
                                }
                            }
                            let _ = send.send((frame.channel, frame.payload));
                        }
                        continue;
                    }
                    if frame.kind != Kind::Data {
                        continue;
                    }
                    let request: Value = serde_json::from_slice(&frame.payload)?;
                    let mut channel = None;
                    let terminal = |id: &str| {
                        let mut t = json!({"address":id,"labels":doc.labels(id),"argv":["sh"],"cwd":"/tmp","state":"live","created_ms":1,"screen":"","cols":80,"rows":24});
                        if qa_statuses()
                            && let Some(status) = qa_status(id)
                        {
                            t["status"] = status;
                        }
                        t
                    };
                    if request["method"] == "terminal.put" {
                        let bytes = STANDARD
                            .decode(request["params"]["data"].as_str().unwrap_or_default())?;
                        let (index, reply) = probe.reply(&request, &bytes);
                        #[cfg(all(target_os = "ios", target_abi = "sim"))]
                        record_put(&request, bytes.len());
                        match reply {
                            Some(reply) => {
                                send_put_reply(&mut server, &request, reply)?;
                                #[cfg(test)]
                                probe
                                    .replies_written
                                    .fetch_add(1, std::sync::atomic::Ordering::Release);
                            }
                            None => {
                                pending.insert(index, request);
                            }
                        }
                        continue;
                    }
                    let method = request["method"].as_str().unwrap_or_default();
                    if let Some(answer) = doc.call(method, &request["params"]) {
                        let reply = match answer {
                            Ok(result) => {
                                json!({"jsonrpc":"2.0","id":request["id"],"result":result})
                            }
                            Err(error) => json!({"jsonrpc":"2.0","id":request["id"],"error":error}),
                        };
                        server.write_all(&encode(0, Kind::Data, reply.to_string().as_bytes())?)?;
                        continue;
                    }
                    let result = match request["method"].as_str() {
                        Some("terminal.list") => {
                            let ids = namespace
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            json!({"terminals":ids.iter().map(|id|terminal(id)).collect::<Vec<_>>()})
                        }
                        Some("terminal.run") => {
                            let mut ids = namespace
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            let mut next = 1;
                            while ids.contains(&format!("t{next}")) {
                                next += 1;
                            }
                            let id = probe
                                .spawn_address
                                .clone()
                                .unwrap_or_else(|| format!("t{next}"));
                            ids.push(id.clone());
                            doc.record_labels(&id, &request["params"]["labels"]);
                            terminal(&id)
                        }
                        Some("terminal.kill") => {
                            let id = request["params"]["terminal"]
                                .as_str()
                                .ok_or(crate::Error::CoreState)?;
                            namespace
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .retain(|address| address != id);
                            #[cfg(test)]
                            let _ = send.send((0, frame.payload.clone()));
                            let mut result = terminal(id);
                            result["state"] = json!("finished");
                            result
                        }
                        Some("terminal.attach") => {
                            let id = request["params"]["terminal"].as_str().unwrap_or("t1");
                            if !namespace
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .iter()
                                .any(|known| known == id)
                            {
                                return Err(crate::Error::CoreState.into());
                            }
                            let number = next_channel;
                            next_channel += 1;
                            channels.insert(number, id.to_owned());
                            channel = Some(number);
                            let mut t = terminal(id);
                            t["cols"] = request["params"]["cols"].clone();
                            t["rows"] = request["params"]["rows"].clone();
                            json!({"channel":number,"terminal":t})
                        }
                        Some("events.subscribe") => {
                            let number = next_channel;
                            next_channel += 1;
                            subscriptions.insert(number);
                            seen = doc.event_count();
                            let ids = namespace
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            json!({"channel":number,"seq":0,"terminals":ids.iter().map(|id|terminal(id)).collect::<Vec<_>>()})
                        }
                        _ => json!({}),
                    };
                    let reply = json!({"jsonrpc":"2.0","id":request["id"],"result":result});
                    server.write_all(&encode(0, Kind::Data, reply.to_string().as_bytes())?)?;
                    if let Some(channel) = channel {
                        let sh1_152 = cfg!(target_abi = "sim")
                            && std::env::var_os("YEBISU_SH1_152_CASE").is_some();
                        let modes = format!(
                            "{}{}",
                            if report_all { "\x1b[>11u" } else { "" },
                            if probe.bracketed || sh1_152 {
                                "\x1b[?2004h"
                            } else {
                                ""
                            }
                        );
                        // L66: the link fixture, only with YEBISU_URL_TAP_QA.
                        let links = if crate::terminal_links::qa::enabled() {
                            let cols = request["params"]["cols"].as_u64().unwrap_or(80);
                            crate::terminal_links::qa::herdr2_output(
                                u16::try_from(cols).unwrap_or(80),
                            )
                        } else {
                            String::new()
                        };
                        let output = format!(
                            "{modes}\x1b[2J\x1b[H\x1b[38;2;136;192;208myebisu P9 Herdr 2\r\nprivate core wire peer · {} · pane {channel}\r\n日本語 IME · Split · Detach / Reattach\x1b[0m{links}\r\n$ ",
                            channels
                                .get(&channel)
                                .map(String::as_str)
                                .unwrap_or("unknown")
                        );
                        server.write_all(&encode(channel, Kind::Data, output.as_bytes())?)?;
                    }
                }
            })();
            if let Err(e) = result {
                eprintln!("private core peer ended: {e}");
            }
        });
        #[cfg(test)]
        let client = transport::Guarded::new(client, gate.unwrap_or_default());
        Ok((CoreBackend::new(Core::over(client)?, 10000), receive))
    }
}
#[cfg(all(target_os = "ios", target_abi = "sim"))]
pub(crate) fn connect() -> crate::Result<CoreBackend> {
    static HOST: std::sync::OnceLock<Host> = std::sync::OnceLock::new();
    HOST.get_or_init(Host::default)
        .recorded(false)
        .map(|(backend, _)| backend)
}

fn send_put_reply(
    server: &mut UnixStream,
    request: &Value,
    receipt: Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let reply = if let Some(error) = receipt.get("error") {
        json!({"jsonrpc":"2.0","id":request["id"],"error":error})
    } else {
        json!({"jsonrpc":"2.0","id":request["id"],"result":receipt})
    };
    server.write_all(&encode(0, Kind::Data, reply.to_string().as_bytes())?)?;
    Ok(())
}
#[cfg(all(target_os = "ios", target_abi = "sim"))]
fn record_put(request: &Value, decoded: usize) {
    if let Some(home) = std::env::var_os("HOME") {
        let path = std::path::PathBuf::from(home).join("Documents/p9-core-wire.jsonl");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = crate::observation::write_jsonl_line(
                &mut file,
                &json!({"method":"terminal.put","terminal":request["params"]["terminal"],"name":request["params"]["name"],"decoded":decoded}),
            );
        }
    }
}
#[cfg(test)]
pub(crate) fn controlled(
    probe: paste::Probe,
    control: transport::Control,
) -> crate::Result<(CoreBackend, Receipts)> {
    Host::default().with_transport(false, probe, Some(control))
}

#[cfg(test)]
pub(crate) fn named(
    mut probe: paste::Probe,
    address: &str,
) -> crate::Result<(CoreBackend, Receipts)> {
    probe.spawn_address = Some(address.to_owned());
    let host = Host {
        terminals: Arc::new(Mutex::new(Vec::new())),
        doc: doc::Store::default(),
    };
    host.with_paste(false, probe)
}
#[cfg(test)]
impl Host {
    /// The host's documents, for tests to change and read.
    pub(crate) fn doc(&self) -> &doc::Store {
        &self.doc
    }
}
