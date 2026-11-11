use super::{Values, store};
use crate::Result;
use std::{path::PathBuf, sync::mpsc, thread};

pub(super) struct Request {
    pub revision: u64,
    pub values: Values,
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
                let result = store::save(&root, &request.values);
                if done
                    .send(Completion::Saved(request.revision, result))
                    .is_err()
                {
                    break;
                }
            }
        })?;
    Ok((send, replies))
}
