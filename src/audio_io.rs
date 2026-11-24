//! Microphone capture through PipeWire and decoding of audio files, both to 16 kHz mono f32.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::endpoint::Endpointer;

pub const SAMPLE_RATE: usize = 16_000;

/// What the capture thread has seen so far. Time is measured in samples, so the auto-stop
/// decision does not depend on thread timing.
#[derive(Default)]
pub struct Capture {
    pub samples: Vec<f32>,
    pub speech_started: bool,
    /// Sample count at the end of the last voiced chunk.
    pub last_voiced: usize,
    pub error: Option<String>,
}

pub struct Recorder {
    child: Child,
    reader: Option<std::thread::JoinHandle<()>>,
    pub capture: Arc<Mutex<Capture>>,
}

impl Recorder {
    /// `source` is a PipeWire target (node name or serial); empty = the default microphone.
    /// A name ending in `.monitor` records what that sink is playing.
    pub fn start(source: &str) -> Result<Recorder, String> {
        let mut command = Command::new("pw-record");
        command.args(["--rate", "16000", "--channels", "1", "--format", "f32", "--latency", "40ms", "--raw"]);
        if let Some(sink) = source.strip_suffix(".monitor") {
            command.args(["--target", sink, "-P", "{ stream.capture.sink = true }"]);
        } else if !source.is_empty() {
            command.args(["--target", source]);
        }
        command.arg("-").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|e| format!("cannot start pw-record ({e}); is pipewire-bin installed?"))?;
        let mut stdout = child.stdout.take().ok_or("pw-record has no output")?;
        let mut stderr = child.stderr.take();
        let capture = Arc::new(Mutex::new(Capture::default()));
        let shared = capture.clone();
        let reader = std::thread::spawn(move || {
            let mut endpointer = Endpointer::new();
            let mut pending: Vec<u8> = Vec::new();
            let mut buffer = [0u8; 8192];
            loop {
                let read = match stdout.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => read,
                };
                pending.extend_from_slice(&buffer[..read]);
                let whole = pending.len() / 4 * 4;
                let chunk: Vec<f32> = pending[..whole]
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect();
                pending.drain(..whole);
                let started = endpointer.push(&chunk);
                let voiced = endpointer.is_voiced();
                let mut capture = shared.lock().unwrap_or_else(|p| p.into_inner());
                capture.samples.extend_from_slice(&chunk);
                capture.speech_started |= started;
                if voiced {
                    capture.last_voiced = capture.samples.len();
                }
            }
            let mut message = String::new();
            if let Some(stderr) = stderr.as_mut() {
                let _ = stderr.read_to_string(&mut message);
            }
            let mut capture = shared.lock().unwrap_or_else(|p| p.into_inner());
            if capture.samples.is_empty() && !message.trim().is_empty() {
                capture.error = Some(message.trim().lines().last().unwrap_or("").to_string());
            }
        });
        Ok(Recorder { child, reader: Some(reader), capture })
    }

    pub fn exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    /// Stops capture and returns everything recorded. SIGINT lets pw-record drain what it has
    /// already taken from the graph, so the end of the last word is not cut off.
    pub fn finish(mut self) -> Result<Vec<f32>, String> {
        unsafe { libc::kill(self.child.id() as i32, libc::SIGINT) };
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                _ if std::time::Instant::now() > deadline => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let mut capture = self.capture.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(error) = capture.error.take() {
            return Err(format!("microphone capture failed: {error}"));
        }
        Ok(std::mem::take(&mut capture.samples))
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn pcm_from_f32le(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
}

fn have(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false)
}

/// Decodes any audio file to 16 kHz mono. A WAV already in that shape is read directly;
/// everything else goes through ffmpeg or, failing that, GStreamer.
pub fn read_file(path: &Path) -> Result<Vec<f32>, String> {
    if !path.is_file() {
        return Err(format!("no such file: {}", path.display()));
    }
    if let Ok(mut reader) = hound::WavReader::open(path) {
        let spec = reader.spec();
        if spec.channels == 1 && spec.sample_rate == SAMPLE_RATE as u32 {
            let samples: Result<Vec<f32>, _> = match (spec.sample_format, spec.bits_per_sample) {
                (hound::SampleFormat::Float, 32) => reader.samples::<f32>().collect(),
                (hound::SampleFormat::Int, 16) => {
                    reader.samples::<i16>().map(|s| s.map(|v| v as f32 / 32768.0)).collect()
                }
                _ => Err(hound::Error::Unsupported),
            };
            if let Ok(samples) = samples {
                return Ok(samples);
            }
        }
    }
    let output = if have("ffmpeg") {
        Command::new("ffmpeg")
            .args(["-v", "error", "-nostdin", "-i"])
            .arg(path)
            .args(["-vn", "-f", "f32le", "-ac", "1", "-ar", "16000", "-"])
            .output()
    } else if have("gst-launch-1.0") {
        Command::new("gst-launch-1.0")
            .arg("-q")
            .arg("filesrc")
            .arg(format!("location={}", path.display()))
            .args(["!", "decodebin", "!", "audioconvert", "!", "audioresample", "!"])
            .arg("audio/x-raw,format=F32LE,rate=16000,channels=1")
            .args(["!", "fdsink", "fd=1"])
            .output()
    } else {
        return Err("this file needs ffmpeg or gst-launch-1.0 to decode (only 16 kHz mono WAV is read directly)".into());
    }
    .map_err(|e| e.to_string())?;
    if output.stdout.len() < 4 {
        let reason = String::from_utf8_lossy(&output.stderr);
        return Err(format!("could not decode audio: {}", reason.trim().lines().last().unwrap_or("no audio stream")));
    }
    Ok(pcm_from_f32le(&output.stdout))
}
