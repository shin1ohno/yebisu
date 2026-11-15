//! Private socketpair peer speaking the pinned core framing and hello protocol.
pub(crate) mod paste;
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

/// The remote terminal namespace outlives any one client connection.
#[derive(Clone)]
pub(crate) struct Host(Arc<Mutex<Vec<String>>>);
impl Default for Host {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(vec!["t1".into(), "t2".into()])))
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
        let namespace = self.0.clone();
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
                loop {
                    let frame: Frame = loop {
                        if probe.close.load(std::sync::atomic::Ordering::Acquire) {
                            return Ok(());
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
                                    let _ = writeln!(
                                        file,
                                        "{}",
                                        json!({"channel":frame.channel,"terminal":channels.get(&frame.channel),"bytes":frame.payload})
                                    );
                                }
                            }
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
                    let terminal = |id: &str| json!({"address":id,"labels":{},"argv":["sh"],"cwd":"/tmp","state":"live","created_ms":1,"screen":"","cols":80,"rows":24});
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
                            terminal(&id)
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
                        _ => json!({}),
                    };
                    let reply = json!({"jsonrpc":"2.0","id":request["id"],"result":result});
                    server.write_all(&encode(0, Kind::Data, reply.to_string().as_bytes())?)?;
                    if let Some(channel) = channel {
                        let modes = format!(
                            "{}{}",
                            if report_all { "\x1b[>11u" } else { "" },
                            if probe.bracketed { "\x1b[?2004h" } else { "" }
                        );
                        let output = format!(
                            "{modes}\x1b[2J\x1b[H\x1b[38;2;136;192;208myebisu P9 Herdr 2\r\nprivate core wire peer · {} · pane {channel}\r\n日本語 IME · Split · Detach / Reattach\x1b[0m\r\n$ ",
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
            let _ = writeln!(
                file,
                "{}",
                json!({"method":"terminal.put","terminal":request["params"]["terminal"],"name":request["params"]["name"],"decoded":decoded})
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
    Host(Arc::new(Mutex::new(Vec::new()))).with_paste(false, probe)
}
