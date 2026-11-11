use super::{Values, store};
use crate::Result;
use std::{path::PathBuf, sync::mpsc, thread};

pub(super) enum Request {
    Save {
        revision: u64,
        values: Values,
    },
    #[cfg(test)]
    Barrier(mpsc::SyncSender<()>),
    #[cfg(test)]
    Pause(mpsc::SyncSender<()>, mpsc::Receiver<()>),
}
pub(super) enum Completion {
    Loaded(Result<Values>),
    Saved(u64, Result<()>),
}
pub(super) fn start(
    root: PathBuf,
) -> Result<(mpsc::SyncSender<Request>, mpsc::Receiver<Completion>)> {
    let (send, receive) = mpsc::sync_channel::<Request>(1);
    let (done, replies) = mpsc::sync_channel(2);
    thread::Builder::new()
        .name("settings-store".into())
        .spawn(move || {
            if done.send(Completion::Loaded(store::load(&root))).is_err() {
                return;
            }
            while let Ok(request) = receive.recv() {
                let (revision, values) = match request {
                    Request::Save { revision, values } => (revision, values),
                    #[cfg(test)]
                    Request::Barrier(ack) => {
                        let _ = ack.send(());
                        continue;
                    }
                    #[cfg(test)]
                    Request::Pause(entered, resume) => {
                        let _ = entered.send(());
                        let _ = resume.recv();
                        continue;
                    }
                };
                let result = store::save(&root, &values);
                if done.send(Completion::Saved(revision, result)).is_err() {
                    break;
                }
            }
        })?;
    Ok((send, replies))
}
