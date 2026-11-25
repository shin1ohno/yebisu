use super::{
    Command, Observations, Queued, SessionInfo, SessionList, SessionTarget, TabInfo,
    doc_tabs::{self, DocTabs},
    herdr_doc_sync::DocMailbox,
    screen::{self, Screen},
    sort_sessions,
    store::{self, Saved, Tab, Workspace},
    vacate::{self, Removal},
};
use crate::{live::Live, shell::single_pane::Mailbox};
use sheprd_backend::{Backend as _, Role, Size, Spawn, SurfaceUpdate, Terminal as _};
use sheprd_core_client::{
    CoreBackend,
    wire::{Run, Terminal as ListedTerminal, TerminalState},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::Receiver,
    },
    time::{Duration, Instant},
};

mod active;

use active::Active;

pub(super) fn run(
    backend: CoreBackend,
    target: SessionTarget,
    generation: u64,
    outputs: (&Arc<Mutex<Live>>, &SessionList, &Mutex<Option<Removal>>),
    controls: (&AtomicBool, &AtomicU64, &Mailbox),
    (commands, doc_mailbox): (Receiver<Queued>, Option<DocMailbox>),
    root: Option<PathBuf>,
) -> crate::Result<()> {
    let SessionTarget {
        host,
        name,
        focus,
        expected_core_id,
    } = target;
    let (inbox, session_list, removal) = outputs;
    // The server has run at least this long when this connection is made.
    let connected_at = Instant::now();
    let (stop, input_epoch, column) = controls;
    let _disconnect = super::paste::Disconnect(backend.clone());
    let mut uploads = super::paste::Uploads::default();
    if expected_core_id
        .as_ref()
        .is_some_and(|id| id != &backend.core().hello().core.id)
    {
        return Err(crate::Error::Ssh(
            "Herdr 2 session changed; tap its refreshed row".into(),
        ));
    }
    let storage = store::LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut workspace = store::load_workspace(root.as_deref(), &host, &name)?;
    if workspace.created == 0 {
        workspace.created = store::milliseconds(std::time::SystemTime::now());
    }
    let available = backend.core().list(false)?.terminals;
    if expected_core_id.is_some() {
        reconcile_live(&mut workspace, &available);
    } else {
        reconcile(&mut workspace, &available);
    }
    // #144 D2: the document decides what is drawn before anything attaches.
    let mut doc = DocTabs::default();
    let mut doc_seen = 0;
    if let Some(snapshot) = doc_mailbox
        .as_ref()
        .and_then(|mailbox| mailbox.first(&mut doc_seen, stop))
    {
        doc.take(snapshot, &mut workspace);
    }
    if let Some(address) = &focus {
        doc.focus(address, &mut workspace);
    }
    if let Some(address) = focus
        && !focus_address(&mut workspace, &address, &available)
    {
        return Err(crate::Error::Ssh(
            "Herdr 2 terminal no longer exists".into(),
        ));
    }
    let size = Size::new(80, 24)?;
    // Attaching first drops what finished while no client was attached.
    let mut active = Active::attach(&backend, &mut workspace, size, column.latest())?;
    if workspace.tabs.is_empty() && !doc.drawing_space() {
        workspace.tabs.push(spawn_tab(&backend)?);
        active = Active::attach(&backend, &mut workspace, size, column.latest())?;
    }
    let mut size = size;
    let mut epoch = 0;
    let mut revision = 0;
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    store::save_workspace(root.as_deref(), &host, &name, &doc.saved(&workspace))?;
    let mut sessions = catalog_sessions(root.as_deref(), &host, Observations::new())?;
    sessions.retain(|row| row.name != name);
    sessions.push(current_info(name.clone(), &workspace, &doc));
    sort_sessions(&mut sessions);
    drop(storage);
    let mut published_epoch = None;
    let stopper = vacate::stopper(&host);
    let mut watch = vacate::Watch::default();
    #[cfg(test)]
    let mut publication_probe: Option<Box<super::publication_probe::AfterCompare>> = None;
    while !stop.load(Ordering::Acquire) {
        // With every pane ended nothing reads a stream, yet a lost link is
        // still the session's end and must reach the view.
        if backend.core().is_lost() {
            return Err(sheprd_core_client::Error::Lost.into());
        }
        let mut changed = false;
        let mut persist = false;
        let mut command_error = None;
        // A new document read: the rows always change, the panes may. Only
        // a change of the drawn panes is a new epoch; input aimed at the
        // surface drawn now must not become stale for a change elsewhere.
        let mut rows_changed = false;
        if let Some(snapshot) = doc_mailbox
            .as_ref()
            .and_then(|mailbox| mailbox.since(&mut doc_seen))
        {
            let (before, view) = (doc_tabs::drawn(&workspace), workspace.doc_view.clone());
            if doc.take(snapshot, &mut workspace) {
                if before != doc_tabs::drawn(&workspace) {
                    active = Active::attach(&backend, &mut workspace, size, column.latest())?;
                    epoch += 1;
                    changed = true;
                }
                persist = view != workspace.doc_view;
                refresh(&mut sessions, &name, &workspace, &doc);
                rows_changed = true;
            }
        }
        for command in commands.try_iter().take(256) {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let queued_epoch = command.input_epoch;
            // The user acts: a failed stop's notice has been seen.
            if !matches!(command.command, Command::Resize(_)) {
                clear_notice(session_list);
            }
            match command.command {
                Command::Input(destination, event) => {
                    let Some(saved) = active_layout(&workspace) else {
                        continue;
                    };
                    let current = screen::publish(
                        &active.screens,
                        saved,
                        size,
                        generation,
                        revision,
                        epoch,
                        column.latest(),
                    );
                    if queued_epoch == input_epoch.load(Ordering::Acquire)
                        && !active.screens[saved.active].ended
                        && crate::terminal_input::destination(&current, generation).as_ref()
                            == Some(&destination)
                    {
                        #[cfg(test)]
                        if let Some(probe) = publication_probe.take() {
                            // Intentionally AFTER the successful comparison. This only
                            // observes the existing race; it adds no cancellation check.
                            probe.hold(&destination, &event, queued_epoch)?;
                        }
                        active.terminals[saved.active].input(&event)?;
                    }
                }
                Command::Wheel {
                    destination,
                    pane,
                    event,
                } => {
                    let Some(saved) = active_layout(&workspace) else {
                        continue;
                    };
                    let current = screen::publish(
                        &active.screens,
                        saved,
                        size,
                        generation,
                        revision,
                        epoch,
                        column.latest(),
                    );
                    if queued_epoch == input_epoch.load(Ordering::Acquire)
                        && crate::terminal_input::destination(&current, generation).as_ref()
                            == Some(&destination)
                        && let Some(index) = saved.ids.iter().position(|id| id.as_str() == pane)
                        && active
                            .screens
                            .get(index)
                            .is_some_and(|screen| !screen.ended)
                        && let Some(terminal) = active.terminals.get_mut(index)
                    {
                        terminal.input(&event)?;
                    }
                }
                #[cfg(test)]
                Command::PauseInput(gate) => gate.wait()?,
                #[cfg(test)]
                Command::ArmPublicationProbe(probe) => {
                    if publication_probe.replace(probe).is_some() {
                        return Err(crate::Error::CoreState);
                    }
                }
                #[cfg(test)]
                Command::FinishPublicationProbe(key, ack) => {
                    if publication_probe.is_some() {
                        return Err(crate::Error::CoreState);
                    }
                    // Fixture-only ordered RPC, followed by producer return. The
                    // receiver still requires transport Drop + full EOF transcript.
                    // A list reply alone is neither a writer fence nor a PTY drain.
                    backend.core().list(true)?;
                    ack.send(key).map_err(|_| crate::Error::CoreState)?;
                    return Ok(());
                }
                Command::PutImage {
                    destination,
                    ticket,
                    bytes,
                    reply,
                } => {
                    let Some(saved) = active_layout(&workspace) else {
                        super::paste::reject(ticket, reply, "Paste cancelled: tab closed");
                        continue;
                    };
                    let current = screen::publish(
                        &active.screens,
                        saved,
                        size,
                        generation,
                        revision,
                        epoch,
                        column.latest(),
                    );
                    if queued_epoch == input_epoch.load(Ordering::Acquire)
                        && !active.screens[saved.active].ended
                        && crate::terminal_input::destination(&current, generation).as_ref()
                            == Some(&destination)
                    {
                        uploads.start(
                            &backend,
                            destination,
                            saved.ids[saved.active].as_str().to_owned(),
                            bytes,
                            ticket,
                            reply,
                        );
                    } else {
                        super::paste::reject(ticket, reply, "Paste cancelled: target changed");
                    }
                }
                Command::Resize(next) => {
                    size = next;
                    changed = true;
                }
                Command::NewTab | Command::Split | Command::SplitInDirection(_)
                    if doc.drawing_space() =>
                {
                    command_error = Some(doc_tabs::NOT_YET.to_owned());
                }
                Command::NewTab => {
                    workspace.tabs.push(spawn_tab(&backend)?);
                    workspace.active = workspace.tabs.len() - 1;
                    active = Active::attach(&backend, &mut workspace, size, column.latest())?;
                    persist = true;
                    changed = true;
                    epoch += 1;
                }
                Command::SelectTab(id) => {
                    if let Some(index) = workspace.tabs.iter().position(|tab| tab.id == id)
                        && index != workspace.active
                    {
                        workspace.active = index;
                        doc.note_tab(&mut workspace);
                        active = Active::attach(&backend, &mut workspace, size, column.latest())?;
                        persist = true;
                        changed = true;
                        epoch += 1;
                    }
                }
                Command::SelectDocSpace(space) => {
                    let before = doc_tabs::drawn(&workspace);
                    doc.select(space, &mut workspace);
                    if before != doc_tabs::drawn(&workspace) {
                        active = Active::attach(&backend, &mut workspace, size, column.latest())?;
                    }
                    persist = true;
                    changed = true;
                    epoch += 1;
                }
                Command::CloseTab(id) => {
                    let Some(index) = workspace.tabs.iter().position(|tab| tab.id == id) else {
                        continue;
                    };
                    let was_active = index == workspace.active;
                    let saved = &mut workspace.tabs[index].layout;
                    let count = saved.ids.len();
                    let focused = saved.ids.get(saved.active).cloned();
                    saved.ids.retain(|pane| match backend.kill(pane) {
                        Ok(()) => false,
                        Err(error) => {
                            command_error = Some(format!("Could not close Herdr 2 tab: {error}"));
                            true
                        }
                    });
                    if saved.ids.len() == count {
                        continue;
                    }
                    if saved.ids.is_empty() {
                        workspace.tabs.remove(index);
                        if index < workspace.active {
                            workspace.active -= 1;
                        } else if index == workspace.active {
                            workspace.active = index.min(workspace.tabs.len().saturating_sub(1));
                        }
                    } else {
                        saved.active = saved
                            .ids
                            .iter()
                            .position(|pane| Some(pane) == focused.as_ref())
                            .unwrap_or(0);
                    }
                    if was_active {
                        active = Active::attach(&backend, &mut workspace, size, column.latest())?;
                    }
                    epoch += 1;
                    persist = true;
                }
                Command::FocusAddress(address) => {
                    let switched = doc.focus(&address, &mut workspace);
                    if let Some((tab, pane)) = locate(&workspace, &address) {
                        if switched || tab != workspace.active {
                            workspace.active = tab;
                            workspace.tabs[tab].layout.active = pane;
                            active =
                                Active::attach(&backend, &mut workspace, size, column.latest())?;
                        } else {
                            let saved = &mut workspace.tabs[tab].layout;
                            active.focus(saved, pane)?;
                        }
                    } else {
                        let available = backend.core().list(false)?.terminals;
                        if !focus_address(&mut workspace, &address, &available) {
                            continue;
                        }
                        active = Active::attach(&backend, &mut workspace, size, column.latest())?;
                    }
                    doc.note_tab(&mut workspace);
                    persist = true;
                    changed = true;
                    epoch += 1;
                }
                Command::Split | Command::SplitInDirection(_) => {
                    let Some(saved) = active_layout_mut(&mut workspace) else {
                        continue;
                    };
                    if saved.ids.len() >= 2 {
                        continue;
                    }
                    let vertical = match command.command {
                        Command::SplitInDirection(vertical) => vertical,
                        _ => saved.vertical,
                    };
                    let info = backend.spawn(&Spawn::default())?;
                    let terminal = backend.attach(
                        &info.id,
                        screen::pane_size(size, 2, vertical, column.latest())?,
                        Role::Active,
                    )?;
                    saved.ids.push(info.id);
                    saved.vertical = vertical;
                    active.terminals.push(terminal);
                    active.screens.push(Screen::default());
                    saved.active = 1;
                    persist = true;
                    epoch += 1;
                    changed = true;
                }
                Command::Focus(index) if index < active.terminals.len() => {
                    let saved = active_layout_mut(&mut workspace).ok_or(crate::Error::CoreState)?;
                    active.focus(saved, index)?;
                    persist = true;
                    epoch += 1;
                    changed = true;
                }
                Command::Orientation => {
                    let Some(saved) = active_layout_mut(&mut workspace) else {
                        continue;
                    };
                    saved.vertical ^= true;
                    epoch += 1;
                    persist = true;
                    changed = true;
                }
                _ => {}
            }
        }
        let saving = (root.as_deref(), host.as_str(), name.as_str());
        if persist {
            if !save(stop, saving, (&workspace, &doc), &mut sessions)? {
                return Ok(());
            }
            changed = true;
        }
        if let Some(saved) = active_layout(&workspace) {
            if changed {
                active.resize(saved, size, column.latest())?;
            }
            for (terminal, screen) in active.terminals.iter_mut().zip(&mut active.screens) {
                if screen.ended {
                    continue;
                }
                for update in terminal.next(Duration::from_millis(8))? {
                    if matches!(update, SurfaceUpdate::Ended(_)) {
                        epoch += 1;
                    }
                    screen.apply(update);
                    changed = true;
                }
            }
        }
        // A pane whose program ended closes at once; the session goes on.
        if let Some(saved) = active_layout_mut(&mut workspace)
            && active.close_ended(saved)?
        {
            active.resize(saved, size, column.latest())?;
            epoch += 1;
            changed = true;
            if !save(stop, saving, (&workspace, &doc), &mut sessions)? {
                return Ok(());
            }
        }
        // The last tab of a named space is gone, closed or its program
        // ended: the space goes with it unless a terminal still runs there.
        let gone = workspace.tabs.is_empty()
            || (workspace.tabs.len() == 1
                && !active.screens.is_empty()
                && active.screens.iter().all(|screen| screen.ended));
        if !vacate::protected(&name) && watch.due(gone, Instant::now()) {
            match vacate_space(&backend, &stopper, &name, connected_at)? {
                Vacated::Busy => watch.busy(Instant::now()),
                Vacated::Failed(message) => {
                    // Kept over every screen until the user acts.
                    set_notice(session_list, &message);
                    command_error = Some(message);
                }
                Vacated::Removed => {
                    forget_space(
                        (root.as_deref(), &host, &name),
                        (inbox, session_list, removal),
                        &mut sessions,
                        epoch + 1,
                    );
                    return Ok(());
                }
            }
        }
        if let Some(saved) = active_layout(&workspace) {
            let current = screen::publish(
                &active.screens,
                saved,
                size,
                generation,
                revision,
                epoch,
                column.latest(),
            );
            let destination = (!active.screens[saved.active].ended)
                .then(|| crate::terminal_input::destination(&current, generation))
                .flatten();
            uploads.drain(
                destination.as_ref(),
                saved.ids[saved.active].as_str(),
                &mut active.terminals[saved.active],
                stop.load(Ordering::Acquire),
            );
        }
        if active.screens.iter().all(|screen| screen.ended) {
            std::thread::sleep(Duration::from_millis(16));
        }
        if changed {
            revision += 1;
            let mut live = active_layout(&workspace).map_or_else(
                || {
                    let mut live = Live::default();
                    live.connected = true;
                    live.dirty = true;
                    live.status = "Herdr 2 · No tabs".into();
                    live.input_epoch = epoch;
                    live
                },
                |saved| {
                    screen::publish(
                        &active.screens,
                        saved,
                        size,
                        generation,
                        revision,
                        epoch,
                        column.latest(),
                    )
                },
            );
            if let Some(error) = command_error {
                live.status = error;
            }
            if let Some(notice) = notice(session_list) {
                live.status = notice;
            }
            if let Ok(mut inbox) = inbox.lock() {
                if published_epoch != Some(epoch) {
                    publish_sessions(session_list, epoch, &sessions);
                    published_epoch = Some(epoch);
                }
                *inbox = live;
            }
        } else if let Some(error) = command_error
            && let Ok(mut inbox) = inbox.lock()
        {
            inbox.status = error;
            inbox.dirty = true;
        }
        // New rows for the epoch already published (see `rows_changed`).
        if rows_changed
            && published_epoch == Some(epoch)
            && let Ok(mut inbox) = inbox.lock()
        {
            publish_sessions(session_list, epoch, &sessions);
            inbox.dirty = true;
        }
    }
    Ok(())
}

enum Vacated {
    /// A terminal still runs in the space: it stays.
    Busy,
    /// The server could not be stopped: the space stays, with this error.
    Failed(String),
    /// The server is stopped.
    Removed,
}

/// Stops the space's server when, in a list taken just now, no terminal has a
/// state other than finished. Only a lost connection is an error here.
fn vacate_space(
    backend: &CoreBackend,
    stopper: &vacate::Stopper,
    name: &str,
    connected_at: Instant,
) -> crate::Result<Vacated> {
    let list = backend.core().list(false)?;
    if !vacate::vacant(0, &list.terminals) {
        return Ok(Vacated::Busy);
    }
    // The stop asks the server again, in the connection it stops through, for
    // its id, its age (a server that restarted since this connection was made
    // is younger) and its live terminals.
    let age = u64::try_from(connected_at.elapsed().as_millis())
        .unwrap_or(u64::MAX)
        .saturating_sub(1000);
    let expect = vacate::Expect {
        core_id: backend.core().hello().core.id.clone(),
        pid: None,
        min_uptime_ms: Some(age),
    };
    Ok(match stopper(name, &expect) {
        Ok(vacate::Stopped::Done) => Vacated::Removed,
        Ok(vacate::Stopped::Busy) => Vacated::Busy,
        Err(error) => {
            tracing::warn!(%error, %name, "Herdr 2 space did not stop");
            Vacated::Failed(format!("Could not remove Herdr 2 space {name}: {error}"))
        }
    })
}

fn notice(lists: &SessionList) -> Option<String> {
    lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .notice
        .clone()
}

fn set_notice(lists: &SessionList, message: &str) {
    lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .notice = Some(message.to_owned());
}

fn clear_notice(lists: &SessionList) {
    lists
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .notice = None;
}

/// The space's server is stopped: its saved layout and rows go, the view is
/// told (`Session::removal`), and the worker ends.
fn forget_space(
    (root, host, name): (Option<&Path>, &str, &str),
    (inbox, session_list, removal): (&Arc<Mutex<Live>>, &SessionList, &Mutex<Option<Removal>>),
    sessions: &mut Vec<SessionInfo>,
    epoch: u64,
) {
    {
        let _storage = store::LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Err(error) = store::remove_workspace(root, host, name) {
            tracing::warn!(%error, %name, "Forget a removed Herdr 2 space");
        }
    }
    let next = {
        let mut lists = session_list
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let next = vacate::next_space(&super::merged_sessions(&lists, name, sessions), name);
        lists.discovery.vacancies.done(name);
        lists.discovery.removed.insert(name.to_owned(), None);
        lists.notice = None;
        if let Some(rows) = &mut lists.discovery.rows {
            rows.retain(|row| row.name != name);
        }
        next
    };
    sessions.retain(|row| row.name != name);
    *removal
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Removal {
        name: name.to_owned(),
        next,
    });
    let mut live = Live::default();
    live.status = format!("Herdr 2 · space {name} removed");
    live.input_epoch = epoch;
    live.dirty = true;
    if let Ok(mut inbox) = inbox.lock() {
        publish_sessions(session_list, epoch, sessions);
        *inbox = live;
    }
}

/// Saves the layout and refreshes the session's catalog row. False when the
/// connection was retired meanwhile: nothing may be saved for it then.
fn save(
    stop: &AtomicBool,
    (root, host, name): (Option<&Path>, &str, &str),
    (workspace, doc): (&Workspace, &DocTabs),
    sessions: &mut [SessionInfo],
) -> crate::Result<bool> {
    let _storage = store::LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if stop.load(Ordering::Acquire) {
        return Ok(false);
    }
    store::save_workspace(root, host, name, &doc.saved(workspace))?;
    refresh(sessions, name, workspace, doc);
    Ok(true)
}

/// The session's own catalog row, with the document's spaces (#144 D2).
fn current_info(name: String, workspace: &Workspace, doc: &DocTabs) -> SessionInfo {
    let mut info = session_info(name, workspace, true, Vec::new());
    doc.fill(&mut info, workspace);
    info
}

fn refresh(sessions: &mut [SessionInfo], name: &str, workspace: &Workspace, doc: &DocTabs) {
    if let Some(entry) = sessions.iter_mut().find(|session| session.name == name) {
        *entry = current_info(name.to_owned(), workspace, doc);
    }
}

fn active_layout(workspace: &Workspace) -> Option<&Saved> {
    workspace.tabs.get(workspace.active).map(|tab| &tab.layout)
}

fn active_layout_mut(workspace: &mut Workspace) -> Option<&mut Saved> {
    workspace
        .tabs
        .get_mut(workspace.active)
        .map(|tab| &mut tab.layout)
}

fn locate(workspace: &Workspace, address: &str) -> Option<(usize, usize)> {
    workspace.tabs.iter().enumerate().find_map(|(tab, value)| {
        value
            .layout
            .ids
            .iter()
            .position(|id| id.as_str() == address)
            .map(|pane| (tab, pane))
    })
}

fn focus_address(workspace: &mut Workspace, address: &str, available: &[ListedTerminal]) -> bool {
    if let Some((tab, pane)) = locate(workspace, address) {
        workspace.active = tab;
        workspace.tabs[tab].layout.active = pane;
        return true;
    }
    let Some(info) = available
        .iter()
        .find(|info| info.address == address && info.state == TerminalState::Live)
    else {
        if available.iter().any(|info| info.address == address) {
            tracing::warn!(%address, "Herdr 2 terminal is no longer running");
        }
        return false;
    };
    let Ok(tab) = new_tab(info) else { return false };
    // If the status poll saw a terminal disappear between list and attach,
    // the worker reports the backend error and keeps the old saved layout.
    workspace.tabs.push(tab);
    workspace.active = workspace.tabs.len() - 1;
    true
}

fn spawn_tab(backend: &CoreBackend) -> crate::Result<Tab> {
    let terminal = backend.core().run(&Run {
        keep: true,
        ..Run::default()
    })?;
    new_tab(&terminal)
}

fn new_tab(info: &ListedTerminal) -> crate::Result<Tab> {
    let id = sheprd_backend::TerminalId::new(info.address.clone())?;
    Ok(Tab {
        id: info.address.clone(),
        label: label(info),
        layout: Saved {
            ids: vec![id],
            ..Saved::default()
        },
    })
}

fn label(info: &ListedTerminal) -> String {
    let program = info.argv.first().and_then(|arg| arg.rsplit('/').next());
    [info.name.as_deref(), info.title.as_deref(), program]
        .into_iter()
        .flatten()
        .map(crate::shell::herdr2_frame::session::one_line)
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| "Shell".to_owned())
}

fn reconcile(workspace: &mut Workspace, available: &[ListedTerminal]) {
    let active = workspace
        .tabs
        .get(workspace.active)
        .map(|tab| tab.id.clone());
    for tab in &mut workspace.tabs {
        let focus = tab.layout.ids.get(tab.layout.active).cloned();
        tab.layout
            .ids
            .retain(|id| available.iter().any(|info| info.address == id.as_str()));
        tab.layout.active = tab
            .layout
            .ids
            .iter()
            .position(|id| Some(id) == focus.as_ref())
            .unwrap_or(0);
        if let Some(info) = available.iter().find(|info| {
            tab.layout
                .ids
                .first()
                .is_some_and(|id| id.as_str() == info.address)
        }) {
            tab.label = label(info);
        }
    }
    workspace.tabs.retain(|tab| !tab.layout.ids.is_empty());
    workspace.active = workspace
        .tabs
        .iter()
        .position(|tab| Some(&tab.id) == active.as_ref())
        .unwrap_or(0);
}

fn reconcile_live(workspace: &mut Workspace, available: &[ListedTerminal]) {
    reconcile(workspace, available);
    let mut seen = workspace
        .tabs
        .iter()
        .flat_map(|tab| tab.layout.ids.iter().map(|id| id.as_str().to_owned()))
        .collect::<std::collections::HashSet<_>>();
    for info in available {
        if info.state == TerminalState::Live
            && !seen.contains(&info.address)
            && let Ok(mut tab) = new_tab(info)
        {
            // The first pane of a surviving split may have exited. Never reuse
            // the split's stored tab id for an independently discovered pane.
            while workspace.tabs.iter().any(|existing| existing.id == tab.id) {
                tab.id.push('+');
            }
            seen.insert(info.address.clone());
            workspace.tabs.push(tab);
        }
    }
}

pub(super) fn catalog_sessions(
    root: Option<&Path>,
    host: &str,
    mut observed: Observations,
) -> crate::Result<Vec<SessionInfo>> {
    let mut names = store::sessions(root, host)?;
    names.extend(observed.keys().cloned());
    names.sort();
    names.dedup();
    let mut rows = names
        .into_iter()
        .map(|name| {
            let mut saved = store::load_workspace(root, host, &name)
                .inspect_err(|error| tracing::warn!(%error, %name, "Read Herdr 2 layout"))
                .unwrap_or_default();
            if let Some(Some(row)) = observed.remove(&name) {
                reconcile_live(&mut saved, &row.terminals);
                let statuses = row
                    .terminals
                    .iter()
                    .filter(|terminal| terminal.state == TerminalState::Live)
                    .map(crate::shell::herdr2_frame::CoreStatus::from)
                    .collect();
                let mut session = session_info(name, &saved, true, statuses);
                session.core_id = Some(row.core_id);
                session
            } else {
                session_info(name, &saved, false, Vec::new())
            }
        })
        .collect::<Vec<_>>();
    sort_sessions(&mut rows);
    Ok(rows)
}

fn session_info(
    name: String,
    workspace: &Workspace,
    confirmed: bool,
    statuses: Vec<crate::shell::herdr2_frame::CoreStatus>,
) -> SessionInfo {
    SessionInfo {
        name,
        confirmed,
        core_id: None,
        statuses,
        created: workspace.created,
        doc: None,
        tabs: workspace
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| TabInfo {
                id: tab.id.clone(),
                label: tab.label.clone(),
                panes: tab
                    .layout
                    .ids
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect(),
                active: index == workspace.active,
                focused_pane: tab.layout.active,
                vertical: tab.layout.vertical,
                auto_name: false,
            })
            .collect(),
    }
}

fn publish_sessions(list: &SessionList, epoch: u64, sessions: &[SessionInfo]) {
    list.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .publish(epoch, sessions.to_vec());
}
