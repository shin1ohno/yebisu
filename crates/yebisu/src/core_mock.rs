//! Private socketpair peer speaking the pinned core framing and hello protocol.
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
        let namespace = self.0.clone();
        let (client, mut server) = UnixStream::pair()?;
        client.set_read_timeout(Some(Duration::from_millis(10)))?;
        server.set_read_timeout(Some(Duration::from_secs(10)))?;
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                let mut line = Vec::new();
                let mut byte = [0];
                while !line.ends_with(b"\n") {
                    server.read_exact(&mut byte)?;
                    line.push(byte[0]);
                }
                let hello: Value = serde_json::from_slice(&line)?;
                let reply = json!({"jsonrpc":"2.0","id":hello["id"],"result":{"generation":1,"version":"0.1.13","framed":true,"features":[],"core":{"id":"c0ffee","name":"private-peer"},"methods":Method::ALL.iter().map(|m|m.as_str()).collect::<Vec<_>>()}});
                server.write_all(format!("{reply}\n").as_bytes())?;
                let mut decoder = Decoder::default();
                let mut buffer = [0; 8192];
                let mut next_channel = 1;
                let mut channels = std::collections::BTreeMap::new();
                loop {
                    let frame: Frame = loop {
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
                            return Ok(());
                        }
                        decoder.push(&buffer[..count]);
                    };
                    if frame.channel != 0 {
                        if frame.kind == Kind::Data {
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
                            let id = format!("t{next}");
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
                        let modes = if report_all { "\x1b[>11u" } else { "" };
                        let bracketed = if cfg!(target_abi = "sim")
                            && std::env::var_os("YEBISU_SH1_152_CASE").is_some()
                        {
                            "\x1b[?2004h"
                        } else {
                            ""
                        };
                        let output = format!(
                            "{modes}{bracketed}\x1b[2J\x1b[H\x1b[38;2;136;192;208myebisu P9 Herdr 2\r\nprivate core wire peer · {} · pane {channel}\r\n日本語 IME · Split · Detach / Reattach\x1b[0m\r\n$ ",
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
