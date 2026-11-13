use super::{
    Command, Queued,
    screen::{self, Screen},
    store,
};
use crate::live::Live;
use sheprd_backend::{Backend as _, Role, Size, Spawn, SurfaceUpdate, Terminal as _};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::Receiver,
    },
    time::Duration,
};

pub(super) fn run(
    backend: sheprd_core_client::CoreBackend,
    host: String,
    generation: u64,
    inbox: &Arc<Mutex<Live>>,
    controls: (&AtomicBool, &AtomicU64),
    commands: Receiver<Queued>,
    root: Option<PathBuf>,
) -> crate::Result<()> {
    let (stop, input_epoch) = controls;
    let storage = store::LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut saved = store::load(root.as_deref(), &host)?;
    let available = backend.terminals()?;
    saved
        .ids
        .retain(|id| available.iter().any(|info| &info.id == id));
    if saved.ids.is_empty() {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        saved.ids.push(backend.spawn(&Spawn::default())?.id);
    }
    saved.active = saved.active.min(saved.ids.len() - 1);
    let mut size = Size::new(80, 24)?;
    let initial = screen::pane_size(size, saved.ids.len(), saved.vertical)?;
    let mut terminals = Vec::new();
    for id in &saved.ids {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        terminals.push(backend.attach(id, initial, Role::Active)?);
    }
    let mut screens: Vec<Screen> = terminals.iter().map(|_| Screen::default()).collect();
    let mut epoch = 0;
    let mut revision = 0;
    if stop.load(Ordering::Acquire) {
        return Ok(());
    }
    store::save(root.as_deref(), &host, &saved)?;
    drop(storage);
    #[cfg(test)]
    let mut publication_probe: Option<Box<super::publication_probe::AfterCompare>> = None;
    while !stop.load(Ordering::Acquire) {
        let mut changed = false;
        let mut persist = false;
        for command in commands.try_iter().take(256) {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let queued_epoch = command.input_epoch;
            match command.command {
                Command::Input(destination, event) => {
                    let current =
                        screen::publish(&screens, &saved, size, generation, revision, epoch);
                    if queued_epoch == input_epoch.load(Ordering::Acquire)
                        && !screens[saved.active].ended
                        && crate::terminal_input::destination(&current, generation).as_ref()
                            == Some(&destination)
                    {
                        #[cfg(test)]
                        if let Some(probe) = publication_probe.take() {
                            // Intentionally AFTER the successful comparison. This only
                            // observes the existing race; it adds no cancellation check.
                            probe.hold(&destination, &event, queued_epoch)?;
                        }
                        terminals[saved.active].input(&event)?;
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
                Command::Resize(next) => {
                    size = next;
                    changed = true;
                }
                Command::Split if saved.ids.len() < 2 => {
                    let info = backend.spawn(&Spawn::default())?;
                    let terminal = backend.attach(
                        &info.id,
                        screen::pane_size(size, 2, saved.vertical)?,
                        Role::Active,
                    )?;
                    saved.ids.push(info.id);
                    terminals.push(terminal);
                    screens.push(Screen::default());
                    saved.active = 1;
                    persist = true;
                    epoch += 1;
                    changed = true;
                }
                Command::Focus(index) if index < terminals.len() => {
                    if !screens[saved.active].ended {
                        terminals[saved.active].focus(false)?;
                    }
                    saved.active = index;
                    persist = true;
                    if !screens[index].ended {
                        terminals[index].focus(true)?;
                    }
                    epoch += 1;
                    changed = true;
                }
                Command::Orientation => {
                    saved.vertical ^= true;
                    persist = true;
                    changed = true;
                }
                _ => {}
            }
        }
        if changed {
            let pane_size = screen::pane_size(size, terminals.len(), saved.vertical)?;
            for (terminal, screen) in terminals.iter_mut().zip(&screens) {
                if !screen.ended {
                    terminal.resize(pane_size)?;
                }
            }
            if persist {
                let _storage = store::LOCK
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                store::save(root.as_deref(), &host, &saved)?;
            }
        }
        for (terminal, screen) in terminals.iter_mut().zip(&mut screens) {
            if screen.ended {
                continue;
            }
            for update in terminal.next(Duration::from_millis(8))? {
                if matches!(update, SurfaceUpdate::Ended(_)) {
                    // CLOSE belongs to this pane, not to the workspace.
                    epoch += 1;
                }
                screen.apply(update);
                changed = true;
            }
        }
        if screens.iter().all(|screen| screen.ended) {
            std::thread::sleep(Duration::from_millis(16));
        }
        if changed {
            revision += 1;
            let live = screen::publish(&screens, &saved, size, generation, revision, epoch);
            if let Ok(mut inbox) = inbox.lock() {
                *inbox = live;
            }
        }
    }
    Ok(())
}
