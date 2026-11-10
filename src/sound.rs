//! Short sounds made up at start: wooden knocks for moves and captures, a chime for a crowning, a chord at the end.

use std::f64::consts::TAU;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

const RATE: f64 = 44100.0;

#[derive(Clone, Copy)]
pub enum Effect {
    Step,
    Jump,
    Crown,
    Pile,
    GameOver,
    Click,
}

type Samples = Vec<f32>;

fn silence(seconds: f64) -> Samples {
    vec![0.0; (seconds * RATE) as usize]
}

fn add(target: &mut Samples, source: &[f32], at: f64, gain: f32) {
    let offset = (at * RATE) as usize;
    if target.len() < offset + source.len() {
        target.resize(offset + source.len(), 0.0);
    }
    for (sum, sample) in target[offset..].iter_mut().zip(source) {
        *sum += sample * gain;
    }
}

/// White noise from -1 to 1 out of a small generator, the same every time for a seed.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        const MODULUS: u64 = 2_147_483_647;
        self.0 = self.0 * 48_271 % MODULUS;
        (self.0 - 1) as f32 / (MODULUS - 2) as f32 * 2.0 - 1.0
    }
}

/// A knock of wood on wood: a short burst of filtered noise and quickly damped resonances.
fn knock(pitch: f64, seed: u64) -> Samples {
    let mut samples = silence(0.16);
    let mut noise = Noise(seed);
    let mut filtered = 0.0f32;
    for (i, sample) in samples.iter_mut().enumerate() {
        let t = i as f64 / RATE;
        filtered += (noise.next() - filtered) * 0.35;
        let burst = f64::from(filtered) * (-t / 0.006).exp();
        let body = 0.55 * (TAU * 620.0 * pitch * t).sin() * (-t / 0.028).exp()
            + 0.3 * (TAU * 1480.0 * pitch * t).sin() * (-t / 0.012).exp()
            + 0.45 * (TAU * 170.0 * pitch * t).sin() * (-t / 0.045).exp();
        let attack = (t / 0.0008).min(1.0);
        *sample = ((burst * 0.9 + body) * attack) as f32;
    }
    samples
}

/// A small bell: inharmonic partials dying away at their own rates.
fn bell(frequency: f64, length: f64) -> Samples {
    // The ratio to the bell's frequency, the gain and the decay of each partial.
    const PARTIALS: [(f64, f64, f64); 4] = [(1.0, 1.0, 1.0), (2.0, 0.45, 0.6), (2.76, 0.3, 0.4), (5.4, 0.12, 0.25)];
    let mut samples = silence(length);
    for (i, sample) in samples.iter_mut().enumerate() {
        let t = i as f64 / RATE;
        let value: f64 = PARTIALS.iter().map(|(ratio, gain, decay)| gain * (TAU * frequency * ratio * t).sin() * (-t / (length * 0.35 * decay)).exp()).sum();
        *sample = (value * (t / 0.004).min(1.0)) as f32;
    }
    samples
}

/// The samples at a volume, their loudest at the volume itself.
fn wave(samples: Samples, volume: f32) -> Arc<[f32]> {
    let peak = samples.iter().fold(1e-6f32, |peak, sample| peak.max(sample.abs()));
    samples.into_iter().map(|sample| (sample / peak * volume).clamp(-1.0, 1.0)).collect()
}

/// The sound being played, and how far it has got, in samples of the sound.
struct Voice {
    wave: Arc<[f32]>,
    position: f64,
}

type SharedVoice = Arc<Mutex<Option<Voice>>>;

/// The stream to the default output device. One sound plays at a time: a new one cuts the last one short.
struct Output {
    _stream: cpal::Stream,
    voice: SharedVoice,
    broken: Arc<AtomicBool>,
}

impl Output {
    fn open() -> Option<Output> {
        let device = cpal::default_host().default_output_device()?;
        let supported = device.default_output_config().ok()?;
        let format = supported.sample_format();
        let config = supported.config();
        let voice = SharedVoice::default();
        let broken = Arc::new(AtomicBool::new(false));
        let stream = match format {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, &voice, &broken),
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, &voice, &broken),
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, &voice, &broken),
            cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config, &voice, &broken),
            cpal::SampleFormat::U8 => build_stream::<u8>(&device, &config, &voice, &broken),
            _ => None,
        }?;
        stream.play().ok()?;
        Some(Output { _stream: stream, voice, broken })
    }
}

fn build_stream<T>(device: &cpal::Device, config: &cpal::StreamConfig, voice: &SharedVoice, broken: &Arc<AtomicBool>) -> Option<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = usize::from(config.channels).max(1);
    let step = RATE / f64::from(config.sample_rate.0);
    let voice = voice.clone();
    let broken = broken.clone();
    let fill = move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
        let mut playing = voice.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for frame in output.chunks_mut(channels) {
            // The sound's rate is turned into the device's by taking the samples between its own.
            let mut value = 0.0;
            if let Some(voice) = playing.as_mut() {
                let index = voice.position as usize;
                if index + 1 < voice.wave.len() {
                    let fraction = (voice.position - index as f64) as f32;
                    value = voice.wave[index] + (voice.wave[index + 1] - voice.wave[index]) * fraction;
                    voice.position += step;
                } else {
                    *playing = None;
                }
            }
            frame.fill(T::from_sample(value));
        }
    };
    device.build_output_stream(config, fill, move |_| broken.store(true, Ordering::Relaxed), None).ok()
}

pub struct SoundBank {
    pub enabled: bool,
    waves: [Arc<[f32]>; 6],
    output: Option<Output>,
    opened: Instant,
}

impl SoundBank {
    pub fn new() -> SoundBank {
        let mut jump = knock(1.15, 2);
        add(&mut jump, &knock(0.9, 3), 0.05, 0.8);

        let mut crown = silence(1.2);
        add(&mut crown, &bell(783.99, 1.0), 0.0, 0.8);
        add(&mut crown, &bell(1046.5, 1.0), 0.09, 0.9);

        let mut chord = silence(1.8);
        for (i, note) in [523.25, 659.25, 783.99, 1046.5].into_iter().enumerate() {
            add(&mut chord, &bell(note, 1.5), i as f64 * 0.11, 0.8);
        }

        let mut click = silence(0.03);
        for (i, sample) in click.iter_mut().enumerate() {
            let t = i as f64 / RATE;
            *sample = ((TAU * 2600.0 * t).sin() * (-t / 0.004).exp()) as f32;
        }

        // In the order of `Effect`.
        let waves = [wave(knock(1.0, 1), 0.55), wave(jump, 0.7), wave(crown, 0.3), wave(knock(1.7, 6), 0.35), wave(chord, 0.35), wave(click, 0.18)];
        SoundBank { enabled: true, waves, output: Output::open(), opened: Instant::now() }
    }

    pub fn play(&mut self, effect: Effect) {
        if !self.enabled {
            return;
        }

        // A device that went away, or was not there, is looked for again, though not at every sound.
        let lost = self.output.as_ref().is_none_or(|output| output.broken.load(Ordering::Relaxed));
        if lost && self.opened.elapsed() > Duration::from_secs(3) {
            self.output = Output::open();
            self.opened = Instant::now();
        }

        if let Some(output) = &self.output {
            let voice = Voice { wave: self.waves[effect as usize].clone(), position: 0.0 };
            *output.voice.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(voice);
        }
    }
}
