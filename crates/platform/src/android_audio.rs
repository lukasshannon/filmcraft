//! AAudio output, using the OS stream rather than a bundled C/C++ audio library.
//! Mixing and blocking writes run on one worker, never the activity/UI thread.
use ndk::audio::{AudioDirection, AudioError, AudioFormat, AudioSharingMode, AudioStreamBuilder, Clockid};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};

pub type Fill = Box<dyn FnMut(&mut [f32], usize) + Send>;

#[derive(Default)]
pub struct Output {
    stop: Arc<AtomicBool>,
    played: Arc<AtomicU64>,
    clock_ready: Arc<AtomicBool>,
    rate: u32,
}

impl Output {
    pub fn start(&mut self, mut fill: Fill) -> Result<u32, String> {
        self.stop();
        self.stop = Arc::new(AtomicBool::new(false));
        self.played = Arc::new(AtomicU64::new(0));
        self.clock_ready = Arc::new(AtomicBool::new(false));
        let (stop, played, ready) = (self.stop.clone(), self.played.clone(), self.clock_ready.clone());
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("Android audio output".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
                    let stream = AudioStreamBuilder::new()
                        .map_err(|e| e.to_string())?
                        .direction(AudioDirection::Output)
                        .format(AudioFormat::PCM_Float)
                        .channel_count(2)
                        .sample_rate(48_000)
                        .sharing_mode(AudioSharingMode::Shared)
                        .buffer_capacity_in_frames(4096)
                        .open_stream()
                        .map_err(|e| e.to_string())?;
                    if stream.channel_count() != 2 || stream.format() != AudioFormat::PCM_Float || stream.sample_rate() <= 0 {
                        return Err("The Android audio device did not accept stereo float output".into());
                    }
                    stream.request_start().map_err(|e| e.to_string())?;
                    let _ = tx.send(Ok(stream.sample_rate() as u32));
                    let mut samples = vec![0.0_f32; 2048];
                    while !stop.load(Ordering::Relaxed) {
                        samples.fill(0.0);
                        fill(&mut samples, 2);
                        let mut offset = 0;
                        while offset < samples.len() && !stop.load(Ordering::Relaxed) {
                            // SAFETY: the stream's format and channel count are checked above.
                            // This initialized f32 buffer remains valid throughout the blocking
                            // call, holds exactly frames*2 samples, and has no concurrent users.
                            let write = unsafe { stream.write(samples.as_ptr().add(offset).cast(), ((samples.len() - offset) / 2) as i32, 100_000_000) };
                            // ndk 0.9's generic status conversion treats positive
                            // AAudioStream_write frame counts as __Unknown errors.
                            // AAudio specifies >=0 as success and <0 as errors.
                            let frames = match write {
                                Ok(frames) => frames,
                                Err(AudioError::__Unknown(frames)) if frames >= 0 => frames as u32,
                                Err(error) => return Err(error.to_string()),
                            };
                            if frames as usize > (samples.len() - offset) / 2 {
                                return Err("Invalid Android audio write size".into());
                            }
                            offset += frames as usize * 2;
                            if let Ok(stamp) = stream.timestamp(Clockid::Monotonic) {
                                played.store(stamp.frame_position.max(0) as u64, Ordering::Relaxed);
                                ready.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    let _ = stream.request_stop();
                    Ok(())
                }));
                ready.store(false, Ordering::Relaxed);
                if !matches!(result, Ok(Ok(()))) {
                    let message = match result {
                        Ok(Err(e)) => e,
                        _ => "Android audio worker failed".into(),
                    };
                    log::error!("{message}");
                    let _ = tx.send(Err(message));
                }
            })
            .map_err(|e| e.to_string())?;
        self.rate = match rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(Ok(rate)) => rate,
            other => {
                self.stop();
                return Err(format!("Android audio could not start: {other:?}"));
            }
        };
        Ok(self.rate)
    }
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.clock_ready.store(false, Ordering::Relaxed);
    }
    pub fn rate(&self) -> u32 {
        if self.rate == 0 { 48_000 } else { self.rate }
    }
    pub fn played(&self) -> Option<u64> {
        self.clock_ready.load(Ordering::Relaxed).then(|| self.played.load(Ordering::Relaxed))
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.stop();
    }
}
