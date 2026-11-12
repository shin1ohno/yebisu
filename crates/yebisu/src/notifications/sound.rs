//! One worker, one waiting sound, built-in assets only; never a remote path.
//! Model: herdr-gpui 398c132a sound/playback.rs; assets documented in NOTICE.
use super::{Failure, Sound};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
struct Job {
    active: Arc<AtomicBool>,
    sound: Sound,
    volume: u8,
    epoch: u64,
}
pub(crate) struct Service {
    sender: SyncSender<Job>,
    epoch: Arc<AtomicU64>,
    results: Receiver<Result<(), Failure>>,
}
impl Service {
    pub fn start() -> Result<Self, Failure> {
        let (sender, jobs) = mpsc::sync_channel::<Job>(1);
        let (results, receiver) = mpsc::sync_channel(1);
        let epoch = Arc::new(AtomicU64::new(0));
        let worker_epoch = epoch.clone();
        std::thread::Builder::new()
            .name("yebisu-notification-sound".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    if worker_epoch.load(Ordering::Acquire) != job.epoch
                        || !job.active.load(Ordering::Acquire)
                    {
                        continue;
                    }
                    let result = play(&job, &worker_epoch);
                    let _ = results.try_send(result);
                }
            })
            .map_err(|_| Failure::Native)?;
        Ok(Self {
            sender,
            epoch,
            results: receiver,
        })
    }
    pub fn cancel(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }
    pub fn submit(&self, sound: Sound, volume: u8, active: Arc<AtomicBool>) -> Result<(), Failure> {
        if volume == 0 {
            return Err(Failure::Muted);
        }
        self.sender
            .try_send(Job {
                active,
                sound,
                volume: volume.min(100),
                epoch: self.epoch.load(Ordering::Acquire),
            })
            .map_err(|_| Failure::Busy)
    }
    pub fn result(&self) -> Option<Result<(), Failure>> {
        self.results.try_recv().ok()
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.cancel();
    }
}
#[cfg(target_os = "ios")]
fn play(job: &Job, epoch: &AtomicU64) -> Result<(), Failure> {
    use rodio::{Decoder, DeviceSinkBuilder, Player, Source};
    use std::{
        io::Cursor,
        time::{Duration, Instant},
    };
    let bytes: &'static [u8] = match job.sound {
        Sound::Done => include_bytes!("../../../../assets/sounds/done.mp3"),
        Sound::Request => include_bytes!("../../../../assets/sounds/request.mp3"),
    };
    let decoder = Decoder::try_from(Cursor::new(bytes)).map_err(|_| Failure::Native)?;
    let (error_sender, errors) = mpsc::sync_channel(1);
    let output = DeviceSinkBuilder::from_default_device()
        .map_err(|_| Failure::Native)?
        .with_error_callback(move |_| {
            let _ = error_sender.try_send(());
        })
        .open_sink_or_fallback()
        .map_err(|_| Failure::Native)?;
    let player = Player::connect_new(output.mixer());
    let volume = f32::from(job.volume) / 100.;
    player.set_volume(volume * volume);
    if epoch.load(Ordering::Acquire) != job.epoch || !job.active.load(Ordering::Acquire) {
        return Err(Failure::Stale);
    }
    player.append(decoder.take_duration(Duration::from_secs(15)));
    let start = Instant::now();
    let result = loop {
        if epoch.load(Ordering::Acquire) != job.epoch || !job.active.load(Ordering::Acquire) {
            break Err(Failure::Stale);
        }
        if errors.try_recv().is_ok() || start.elapsed() > Duration::from_secs(15) {
            break Err(Failure::Native);
        }
        if player.empty() {
            break Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    player.stop();
    result
}
#[cfg(not(target_os = "ios"))]
fn play(job: &Job, _epoch: &AtomicU64) -> Result<(), Failure> {
    let _ = (job.sound, job.volume);
    // Host models must never pretend to emit native audio.
    Err(Failure::Native)
}
