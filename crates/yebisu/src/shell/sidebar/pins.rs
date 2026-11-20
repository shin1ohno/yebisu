//! Agent pins are scoped to a host, Herdr kind, daemon boot and pane.
use super::{AgentIdentity, HerdrKind};
use crate::{Error, Result, app::Smoke};
use gpui::Context;
use serde_json::Value;
use sheprd_client::protocol::ClientShellSnapshot;
use std::{
    fs::{self, File},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

const FILE: &str = "yebisu-agent-pins.json";
const MAX_FILE: u64 = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pin {
    id: AgentIdentity,
    boot: String,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Pins {
    newest_first: Vec<Pin>,
    revision: u64,
}

impl Pins {
    pub(super) fn index(&self, id: &AgentIdentity, boot: &str) -> Option<usize> {
        self.newest_first
            .iter()
            .position(|p| p.id == *id && p.boot == boot)
    }

    fn toggle(&mut self, id: AgentIdentity, boot: String) {
        if let Some(index) = self.index(&id, &boot) {
            self.newest_first.remove(index);
        } else {
            self.newest_first.insert(0, Pin { id, boot });
        }
        self.revision += 1;
    }

    pub(super) fn reconcile(&mut self, host: &str, snapshot: &ClientShellSnapshot) {
        let len = self.newest_first.len();
        self.newest_first.retain(|pin| {
            pin.id.host != host
                || pin.id.kind != HerdrKind::Herdr1
                || (pin.boot == snapshot.boot_id
                    && snapshot
                        .panes
                        .iter()
                        .any(|pane| pane.pane_id == pin.id.pane))
        });
        self.revision += u64::from(self.newest_first.len() != len);
    }

    fn load(root: &Path) -> Result<Self> {
        let mut file = match File::open(root.join(FILE)) {
            Ok(file) => file.take(MAX_FILE + 1),
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FILE {
            return Err(invalid());
        }
        let pins: Vec<Value> = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        let mut newest_first = Vec::with_capacity(pins.len());
        for pin in pins {
            let string = |key| {
                pin[key]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(invalid)
            };
            let kind = match pin["herdr"].as_u64() {
                Some(1) => HerdrKind::Herdr1,
                Some(2) => HerdrKind::Herdr2,
                _ => return Err(invalid()),
            };
            let id = AgentIdentity {
                host: string("host")?,
                kind,
                pane: string("pane")?,
            };
            newest_first.push(Pin {
                id,
                boot: string("boot")?,
            });
        }
        Ok(Self {
            newest_first,
            revision: 0,
        })
    }

    fn save(&self, root: &Path) -> Result<()> {
        let records = self
            .newest_first
            .iter()
            .map(|pin| {
                let kind = match pin.id.kind {
                    HerdrKind::Herdr1 => 1,
                    HerdrKind::Herdr2 => 2,
                };
                serde_json::json!({
                    "host": pin.id.host, "herdr": kind, "pane": pin.id.pane, "boot": pin.boot,
                })
            })
            .collect::<Vec<_>>();
        let bytes = serde_json::to_vec(&records).map_err(|_| invalid())?;
        if bytes.len() as u64 > MAX_FILE {
            return Err(invalid());
        }
        fs::create_dir_all(root)?;
        let pending = root.join("yebisu-agent-pins.pending");
        let result = (|| -> Result<()> {
            let mut file = File::create(&pending)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&pending, root.join(FILE))?;
            File::open(root)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(pending);
        }
        result
    }
}

fn invalid() -> Error {
    Error::Settings("Invalid sidebar pin data".into())
}

#[derive(Debug, Default)]
pub(super) enum Store {
    #[default]
    Initial,
    Loading,
    Ready {
        root: PathBuf,
        saved: u64,
        writing: bool,
    },
    Unavailable,
}

impl Smoke {
    /// Keeps pins loaded and reconciled even while the sidebar header is hidden.
    pub(crate) fn load_sidebar_agent_pins(&mut self, cx: &mut Context<Self>) {
        self.flush_sidebar_pins(cx);
    }

    pub(crate) fn toggle_sidebar_agent_pin(
        &mut self,
        id: AgentIdentity,
        boot: &str,
        generation: u64,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let terminal = if id.host == self.terminal.reconnect.target {
            Some(&self.terminal)
        } else {
            self.board.hidden.get(&id.host)
        };
        let current = terminal
            .filter(|terminal| {
                id.kind == HerdrKind::Herdr1
                    && !terminal.core_mode
                    && terminal.live.connected
                    && terminal.link.generation == generation
            })
            .and_then(|t| crate::board::snapshot(&t.live));
        if !current.is_some_and(|s| {
            s.boot_id == boot && s.panes.iter().any(|pane| pane.pane_id == id.pane)
        }) {
            return Err(Error::NativeMenu("Agent pane changed; reopen the menu"));
        }
        let state = &mut self.shell.sidebar;
        if !cfg!(all(test, not(target_os = "ios")))
            && !matches!(state.pin_store, Store::Ready { .. })
        {
            return Err(Error::NativeMenu("Sidebar pins are unavailable"));
        }
        state.pins.borrow_mut().toggle(id, boot.to_owned());
        self.flush_sidebar_pins(cx);
        cx.notify();
        Ok(())
    }

    pub(super) fn flush_sidebar_pins(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.shell.sidebar;
        match &mut state.pin_store {
            Store::Initial => {
                if cfg!(all(test, not(target_os = "ios"))) {
                    state.pin_store = Store::Unavailable;
                    return;
                }
                let Some(root) = sheprd_client::local_path::state_dir() else {
                    state.pin_store = Store::Unavailable;
                    return;
                };
                state.pin_store = Store::Loading;
                let path = root.clone();
                let load = cx
                    .background_executor()
                    .spawn(async move { Pins::load(&path) });
                cx.spawn(async move |weak, cx| {
                    let result = load.await;
                    let _ = weak.update(cx, |this, cx| {
                        let state = &mut this.shell.sidebar;
                        match result {
                            Ok(pins) => {
                                state.pins.replace(pins);
                                state.pin_store = Store::Ready {
                                    root,
                                    saved: 0,
                                    writing: false,
                                };
                            }
                            Err(error) => {
                                state.pin_store = Store::Unavailable;
                                this.terminal.live.status = error.to_string();
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            Store::Ready {
                root,
                saved,
                writing,
            } => {
                let pins = state.pins.borrow();
                if *writing || *saved == pins.revision {
                    return;
                }
                *writing = true;
                let root = root.clone();
                let revision = pins.revision;
                let pending = pins.clone();
                let save = cx
                    .background_executor()
                    .spawn(async move { pending.save(&root) });
                cx.spawn(async move |weak, cx| {
                    let result = save.await;
                    let _ = weak.update(cx, |this, cx| {
                        if let Store::Ready { saved, writing, .. } =
                            &mut this.shell.sidebar.pin_store
                        {
                            *writing = false;
                            // A further edit schedules another write even if this one failed.
                            *saved = revision;
                        }
                        if let Err(error) = result {
                            this.terminal.live.status = error.to_string();
                        }
                        this.flush_sidebar_pins(cx);
                        cx.notify();
                    });
                })
                .detach();
            }
            Store::Loading | Store::Unavailable => {}
        }
    }
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod tests;
