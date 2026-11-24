//! Loads and serves the speech-to-text engine (transcribe.cpp, GGUF models).
//!
//! Desktop port of upstream `src/engine.rs` (offline-speech-to-text, MIT, © 2025 Noah Mühl):
//! the model/language policy, the Whisper run options, bounded overlapping windows for long
//! audio and the final cleanup pass are kept; the JNI layer, the process-wide singleton and the
//! phone-specific thread heuristic are gone, and a compute device can be chosen.

use std::path::{Path, PathBuf};
use transcribe_cpp as tc;

use crate::language_selection::ModelLanguagePolicy;
use crate::transcript_cleanup::OutputLanguage;

const CANARY_FLASH_LANGUAGE_REQUIRED: &str =
    "Choose a supported language for Canary 180M Flash (ostt config set language <code>).";

#[derive(Clone, Debug)]
pub struct EngineOptions {
    pub model: PathBuf,
    pub language: Option<String>,
    pub languages: Vec<String>,
    pub translate: bool,
    pub threads: i32,
    pub device: String,
    pub langid_model: PathBuf,
    pub terms_path: PathBuf,
    pub corrections_path: PathBuf,
}

/// Picks the spoken language among the user's own languages, which plain automatic detection
/// cannot be told (closely related languages are easily mistaken for each other).
struct LanguageDetector {
    session: tc::LangIdSession,
    allowed: Vec<String>,
    previous: String,
    min_samples: usize,
}

impl LanguageDetector {
    fn open(path: &Path, languages: &[String], threads: i32) -> Result<LanguageDetector, String> {
        let model = std::panic::catch_unwind(|| tc::Model::load(path))
            .map_err(|_| "language model loader rejected this file".to_string())?
            .map_err(|e| e.to_string())?;
        let info = model.langid_info().map_err(|e| e.to_string())?;
        let session = model
            .langid_session_with(&tc::LangIdSessionOptions { n_threads: threads })
            .map_err(|e| e.to_string())?;
        Ok(LanguageDetector {
            session,
            allowed: languages
                .iter()
                .map(|code| crate::language_selection::detector_label(code).to_string())
                .collect(),
            previous: crate::language_selection::recognition_code(&languages[0]).to_string(),
            min_samples: (info.min_audio_ms.max(0) as usize).saturating_mul(16),
        })
    }

    fn detect(&mut self, samples: &[f32]) -> String {
        if samples.len() >= self.min_samples {
            let options = tc::LangIdOptions { allowed: Some(self.allowed.clone()) };
            if let Ok(result) = self.session.run(samples, &options) {
                if let Some(code) = result.code().filter(|code| self.allowed.iter().any(|a| a == code)) {
                    self.previous = crate::language_selection::recognition_code(code).to_string();
                }
            }
        }
        self.previous.clone()
    }
}

pub struct Engine {
    pub model_path: PathBuf,
    pub arch: String,
    pub backend: String,
    pub threads: i32,
    /// Primary language codes the model reports; empty when it reports none.
    pub languages: Vec<String>,
    /// Empty, or a note when a setting cannot do what it says with this model.
    pub note: &'static str,
    session: tc::Session,
    language: Option<String>,
    requires_language: bool,
    detector: Option<LanguageDetector>,
    task: tc::Task,
    english_only_model: bool,
    translates_to_english: bool,
    run_ext: Option<tc::RunExtension>,
    supports_initial_prompt: bool,
    terms_path: PathBuf,
    corrections_path: PathBuf,
    /// `None` means the runtime preserves long-input context itself (Whisper). Other offline
    /// families use overlapping bounded windows so memory cannot grow with the recording.
    bounded_window_samples: Option<usize>,
}

/// `auto` prefers an integrated GPU: for speech models it is about as fast as a discrete one,
/// and a background dictation service should not hold VRAM or keep a discrete GPU awake.
pub fn pick_device(wanted: &str) -> Result<(tc::Backend, Option<tc::Device>), String> {
    let wanted = wanted.trim();
    if wanted.eq_ignore_ascii_case("cpu") {
        return Ok((tc::Backend::Cpu, None));
    }
    let devices = tc::devices();
    let chosen = if wanted.is_empty() || wanted.eq_ignore_ascii_case("auto") {
        let by_type = |kind: tc::DeviceType| devices.iter().find(|d| d.device_type == kind).cloned();
        match by_type(tc::DeviceType::Igpu).or_else(|| by_type(tc::DeviceType::Gpu)) {
            Some(device) => device,
            None => return Ok((tc::Backend::Cpu, None)),
        }
    } else {
        let lower = wanted.to_lowercase();
        devices
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(wanted))
            .or_else(|| devices.iter().find(|d| d.description.to_lowercase().contains(&lower)))
            .cloned()
            .ok_or_else(|| format!("no compute device matches '{wanted}' (see `ostt devices`)"))?
    };
    if chosen.device_type == tc::DeviceType::Cpu {
        return Ok((tc::Backend::Cpu, None));
    }
    let backend = match chosen.kind.as_str() {
        "vulkan" => tc::Backend::Vulkan,
        "cuda" => tc::Backend::Cuda,
        "metal" => tc::Backend::Metal,
        "rocm" => tc::Backend::Rocm,
        _ => tc::Backend::Auto,
    };
    Ok((backend, Some(chosen)))
}

/// Desktop CPUs are not the phone SoCs the upstream two-thread cap was measured on: use the
/// physical cores, bounded so a dictation never saturates the machine.
fn default_threads() -> i32 {
    let logical = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    (logical / 2).clamp(2, 8) as i32
}

impl Engine {
    pub fn load(options: &EngineOptions) -> Result<Engine, String> {
        if !options.model.is_file() {
            return Err(format!("model file not found: {}", options.model.display()));
        }
        let threads = if options.threads > 0 { options.threads } else { default_threads() };
        let (backend, device) = pick_device(&options.device)?;
        let model_options = tc::ModelOptions { backend, device, no_mmap: false };
        // Imported GGUF files are untrusted input: contain a loader panic instead of aborting.
        let model = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tc::Model::load_with(&options.model, &model_options)
        }))
        .map_err(|_| "model loader rejected this file".to_string())?
        .map_err(|e| e.to_string())?;

        let capabilities = model.capabilities().ok();
        let translate = options.translate;
        let task = if translate && capabilities.as_ref().is_some_and(|c| c.supports_translate) {
            tc::Task::Translate
        } else {
            tc::Task::Transcribe
        };
        let english_only_model = capabilities
            .as_ref()
            .is_some_and(|c| crate::language_selection::english_only(&c.languages));
        let language_policy = crate::language_selection::model_policy(
            &model.arch(),
            capabilities.as_ref().map(|c| c.languages.as_slice()),
            &options.languages,
        )?;
        let language = if language_policy == ModelLanguagePolicy::EnglishOnly {
            Some("en".to_string())
        } else {
            options.language.clone()
        };
        // Canary Flash defaults to English, which can translate other speech even when the
        // requested task is transcription.
        let requires_language = model.variant() == "canary-180m-flash";
        if requires_language && language_policy != ModelLanguagePolicy::Detect {
            let chosen = language
                .as_deref()
                .map(crate::language_selection::parse_allowed)
                .unwrap_or_default();
            if chosen.len() != 1
                || !capabilities
                    .as_ref()
                    .is_some_and(|caps| crate::language_selection::supports_all(&caps.languages, &chosen))
            {
                return Err(CANARY_FLASH_LANGUAGE_REQUIRED.into());
            }
        }
        let note = if language_policy == ModelLanguagePolicy::IgnoresList {
            "this model ignores the `languages` list"
        } else if language_policy == ModelLanguagePolicy::EnglishOnly {
            "this model supports English only"
        } else if translate && task == tc::Task::Transcribe {
            "this model can't translate"
        } else if translate && model.variant().contains("turbo") {
            "Whisper Turbo translates poorly; use another Whisper"
        } else {
            ""
        };
        // Whisper's stock recipe re-decodes a chunk at up to five higher temperatures when its
        // quality gates fail. For interactive use a single greedy pass is the better trade.
        let is_whisper = model.accepts_ext(tc::ExtSlot::Run, tc::sys::TRANSCRIBE_EXT_KIND_WHISPER_RUN);
        let supports_initial_prompt = is_whisper && model.supports(tc::Feature::InitialPrompt);
        let translates_to_english = task == tc::Task::Translate && !model.variant().contains("turbo");
        let run_ext = is_whisper.then(|| {
            tc::RunExtension::Whisper(tc::WhisperRunOptions {
                temperature_inc: Some(0.0),
                ..Default::default()
            })
        });

        let session = model
            .session_with(&tc::SessionOptions { n_threads: threads, ..Default::default() })
            .map_err(|e| e.to_string())?;
        let bounded_window_samples = if is_whisper {
            None
        } else {
            let native_limit = session
                .limits()
                .ok()
                .map(|limits| limits.effective_max_audio_ms)
                .filter(|limit| *limit > 0)
                .map(|limit| (limit as usize).saturating_mul(crate::long_audio::SAMPLE_RATE) / 1000);
            let safe_limit = native_limit
                .unwrap_or(crate::long_audio::DEFAULT_MAX_WINDOW_SAMPLES)
                .min(crate::long_audio::DEFAULT_MAX_WINDOW_SAMPLES);
            let final_silence = crate::audio_finalization::final_decode_silence_samples();
            if safe_limit < final_silence.saturating_add(crate::long_audio::MIN_WINDOW_SAMPLES) {
                return Err("model reports an unusably short audio limit".to_string());
            }
            Some(safe_limit - final_silence)
        };
        // A multi-language choice requires a usable detector; failure must not silently enable
        // unrestricted recognition.
        let detector = if language_policy == ModelLanguagePolicy::Detect && options.languages.len() >= 2 {
            if !options.langid_model.is_file() {
                return Err("Download the language detector (ostt download langid) or choose a single language".into());
            }
            Some(LanguageDetector::open(&options.langid_model, &options.languages, threads).map_err(|_| {
                "Language detector unavailable; download it again or choose one language".to_string()
            })?)
        } else {
            None
        };

        let mut languages: Vec<String> = Vec::new();
        for tag in capabilities.iter().flat_map(|caps| caps.languages.iter()) {
            for code in crate::language_selection::parse_allowed(tag) {
                if !languages.contains(&code) {
                    languages.push(code);
                }
            }
        }
        let mut engine = Engine {
            model_path: options.model.clone(),
            languages,
            arch: model.arch(),
            backend: model.backend(),
            threads,
            note,
            session,
            language,
            requires_language,
            detector,
            task,
            english_only_model,
            translates_to_english,
            run_ext,
            supports_initial_prompt,
            terms_path: options.terms_path.clone(),
            corrections_path: options.corrections_path.clone(),
            bounded_window_samples,
        };
        engine.warm_up();
        Ok(engine)
    }

    /// GPU backends compile their pipelines on the first run; pay that at load, not on the
    /// first dictation. The result is discarded and a failure here is not an error.
    fn warm_up(&mut self) {
        let mut seed = 0x2545_f491u32;
        let noise: Vec<f32> = (0..crate::long_audio::SAMPLE_RATE)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                (seed as f32 / u32::MAX as f32 - 0.5) * 0.02
            })
            .collect();
        let options = tc::RunOptions {
            language: self.language.clone(),
            task: self.task,
            family: self.run_ext.clone(),
            ..Default::default()
        };
        let _ = self.session.run(&noise, &options);
    }

    /// Transcribes one complete 16 kHz mono recording. Whisper owns its native long-input
    /// context. Other families stay single-pass for normal dictation and use overlapping,
    /// context-reconciled windows only beyond their safe compute bound.
    pub fn transcribe(&mut self, samples: &[f32]) -> Result<String, String> {
        // Snapshot once per transcription. Only models with prompt support receive a
        // recognition hint, but every model can restore the spelling of a preferred term.
        let preferred_terms = crate::custom_terms::load_terms(&self.terms_path);
        let corrections = load_corrections(&self.corrections_path);
        let prompt = if self.supports_initial_prompt {
            crate::custom_terms::compile_request_prompt(preferred_terms.as_deref(), &[])
        } else {
            None
        };
        let run_ext = self.run_extension(prompt);
        let mut output_language = OutputLanguage::English;
        let mut text = String::new();
        let mut joins = Vec::new();

        if let Some(max_window_samples) = self.bounded_window_samples {
            let mut assembled = String::new();
            let mut window_joins: Vec<usize> = Vec::new();
            for window in crate::long_audio::windows(samples, max_window_samples) {
                let mut audio = samples[window].to_vec();
                crate::audio_finalization::append_final_decode_silence(&mut audio);
                let transcript = self.run(&audio, &run_ext)?;
                if !transcript.text.trim().is_empty() {
                    output_language =
                        output_language.combine(self.output_language(transcript.language.as_deref()));
                }
                let (stitched, outcome) =
                    crate::long_audio::stitch_with_join(&assembled, transcript.text.trim());
                match outcome {
                    crate::long_audio::StitchOutcome::Aligned { truncated_at } => {
                        window_joins.retain(|&join| join < truncated_at)
                    }
                    crate::long_audio::StitchOutcome::Fallback { join } => window_joins.push(join),
                }
                assembled = stitched;
            }
            append_joined_piece(&mut text, &mut joins, &assembled, &window_joins);
        } else {
            let mut audio = samples.to_vec();
            crate::audio_finalization::append_final_decode_silence(&mut audio);
            let transcript = self.run(&audio, &run_ext)?;
            if !transcript.text.trim().is_empty() {
                output_language =
                    output_language.combine(self.output_language(transcript.language.as_deref()));
            }
            append_piece(&mut text, transcript.text.trim());
        }
        Ok(crate::transcript_cleanup::finish_request(
            text,
            output_language,
            preferred_terms.as_deref(),
            corrections.as_deref(),
            &joins,
            &[],
        ))
    }

    fn output_language(&self, detected: Option<&str>) -> OutputLanguage {
        let english = crate::english_fillers::known_english_output(
            detected,
            self.language.as_deref(),
            self.english_only_model,
            self.translates_to_english,
        );
        if english {
            OutputLanguage::English
        } else if detected.is_some() || self.language.is_some() {
            OutputLanguage::Other
        } else {
            OutputLanguage::Unknown
        }
    }

    fn run_extension(&self, prompt: Option<String>) -> Option<tc::RunExtension> {
        self.run_ext.clone().map(|extension| match extension {
            tc::RunExtension::Whisper(mut options) => {
                if self.supports_initial_prompt {
                    options.initial_prompt = prompt;
                }
                tc::RunExtension::Whisper(options)
            }
            other => other,
        })
    }

    /// One model run. A rejected language hint is degraded instead of failing the
    /// transcription: `de-DE` retries as `de`, then as no hint. The degraded value is kept so
    /// later runs skip the rejected attempts.
    fn run(&mut self, samples: &[f32], run_ext: &Option<tc::RunExtension>) -> Result<tc::Transcript, String> {
        let range = crate::endpoint::speech_range(samples);
        let samples = &samples[range];
        if samples.is_empty() {
            return Ok(tc::Transcript::default());
        }
        let mut detected = self.detector.as_mut().map(|detector| detector.detect(samples));
        loop {
            let opts = tc::RunOptions {
                language: detected.clone().or_else(|| self.language.clone()),
                task: self.task,
                family: run_ext.clone(),
                ..Default::default()
            };
            match self.session.run(samples, &opts) {
                Ok(mut transcript) => {
                    if transcript.language.is_none() {
                        transcript.language = detected.take();
                    }
                    return Ok(transcript);
                }
                Err(tc::Error::OutputTruncated { partial: Some(transcript), .. })
                | Err(tc::Error::OutputRepetition { partial: Some(transcript), .. }) => {
                    let mut transcript = *transcript;
                    if transcript.language.is_none() {
                        transcript.language = detected.take();
                    }
                    return Ok(transcript);
                }
                Err(tc::Error::Unsupported(msg)) if detected.is_some() => {
                    return Err(format!("The speech model rejected the chosen language: {msg}"));
                }
                Err(tc::Error::Unsupported(msg))
                    if self.requires_language
                        && self.language.as_deref().is_some_and(|tag| !tag.contains('-')) =>
                {
                    return Err(format!("The speech model rejected the chosen language: {msg}"));
                }
                Err(tc::Error::Unsupported(_)) if self.language.is_some() => {
                    let lang = self.language.take().unwrap();
                    self.language = lang.split_once('-').map(|(primary, _)| primary.to_string());
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    }
}

/// Exact corrections are a plain `heard => written` file; an invalid file is ignored as a whole.
fn load_corrections(path: &Path) -> Option<Vec<crate::personal_corrections::Rule>> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > 8192 {
        return None;
    }
    let raw = std::fs::read_to_string(path).ok()?;
    crate::personal_corrections::parse(&raw).ok().filter(|rules| !rules.is_empty())
}

/// Appends like `append_piece` and moves the piece's join offsets into `target_joins`.
fn append_joined_piece(target: &mut String, target_joins: &mut Vec<usize>, piece: &str, joins: &[usize]) {
    let leading = piece.len() - piece.trim_start().len();
    let trimmed = piece.trim();
    append_piece(target, trimmed);
    let start = target.len() - trimmed.len();
    target_joins.extend(
        joins
            .iter()
            .filter(|&&join| join > leading && join < leading + trimmed.len())
            .map(|&join| start + join - leading),
    );
}

fn append_piece(target: &mut String, piece: &str) {
    let piece = piece.trim();
    if !piece.is_empty() {
        if !target.is_empty() {
            target.push(' ');
        }
        target.push_str(piece);
    }
}
