//! Paths and settings. Settings live in one plain `key = value` file so they can be edited by hand.

use std::collections::BTreeMap;
use std::path::PathBuf;

pub const KEYS: &[(&str, &str, &str)] = &[
    ("model", "", "catalogue id / file name under models/, or an absolute path to a GGUF"),
    ("device", "auto", "auto (integrated GPU, then discrete GPU, then CPU) | cpu | a name from `ostt devices`"),
    ("language", "auto", "language hint such as sk or en-US; auto = let the model decide"),
    ("languages", "", "languages you speak (sk,en); two or more need the language detector"),
    ("translate", "false", "translate speech to English (models that support it, e.g. Whisper)"),
    ("threads", "0", "CPU threads for inference; 0 = automatic"),
    ("inject", "type", "type (wtype) | paste (clipboard + Ctrl+V) | copy (clipboard only) | print"),
    ("append_space", "true", "add a space after inserted text"),
    ("type_delay_ms", "350", "wait before typing so the shortcut's modifier keys are released"),
    ("auto_stop_ms", "0", "stop after this much silence: 0 (off), 2000, 3000 or 5000"),
    ("source", "", "PipeWire capture target; empty = default microphone; NAME.monitor = a sink"),
    ("notify", "all", "all | errors | off"),
];

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(var) {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => home().join(fallback),
    }
}

pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("ostt")
}

pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("ostt")
}

pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("ostt")
}

pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

/// Kept out of `models/` so the detector can never be selected as the speech model.
pub fn langid_path() -> PathBuf {
    data_dir().join("aux-models").join(crate::catalog::LANGUAGE_ID.file)
}

pub fn terms_path() -> PathBuf {
    config_dir().join("custom_terms")
}

pub fn corrections_path() -> PathBuf {
    config_dir().join("personal_corrections")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config")
}

pub fn runtime_dir() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })),
    }
}

pub fn socket_path() -> PathBuf {
    runtime_dir().join("ostt.sock")
}

#[derive(Clone, Debug)]
pub struct Config {
    values: BTreeMap<String, String>,
}

impl Config {
    pub fn load() -> Config {
        let mut values = BTreeMap::new();
        if let Ok(raw) = std::fs::read_to_string(config_path()) {
            for line in raw.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    values.insert(key.trim().to_string(), value.trim().trim_matches('"').to_string());
                }
            }
        }
        Config { values }
    }

    pub fn get(&self, key: &str) -> String {
        if let Some(value) = self.values.get(key) {
            return value.clone();
        }
        KEYS.iter()
            .find(|(name, _, _)| *name == key)
            .map(|(_, default, _)| default.to_string())
            .unwrap_or_default()
    }

    pub fn flag(&self, key: &str) -> bool {
        matches!(self.get(key).to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
    }

    pub fn number(&self, key: &str) -> u64 {
        self.get(key).parse().unwrap_or(0)
    }

    pub fn set(key: &str, value: &str) -> Result<(), String> {
        if !KEYS.iter().any(|(name, _, _)| *name == key) {
            return Err(format!("unknown setting '{key}' (see `ostt config`)"));
        }
        let mut config = Config::load();
        config.values.insert(key.to_string(), value.trim().to_string());
        std::fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
        let mut out = String::from("# ostt settings; `ostt config` lists every key.\n");
        for (key, value) in &config.values {
            out.push_str(&format!("{key} = {value}\n"));
        }
        let tmp = config_path().with_extension("tmp");
        std::fs::write(&tmp, out).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, config_path()).map_err(|e| e.to_string())
    }

    /// The speech model file this configuration selects, if it is installed.
    pub fn model_path(&self) -> Result<PathBuf, String> {
        let chosen = self.get("model");
        if chosen.starts_with('/') {
            let path = PathBuf::from(&chosen);
            return if path.is_file() { Ok(path) } else { Err(format!("model file not found: {chosen}")) };
        }
        let name = if chosen.is_empty() { crate::catalog::DEFAULT_FILE.to_string() } else { chosen };
        let file = match crate::catalog::find(&name) {
            Ok(entry) => entry.file.to_string(),
            Err(_) if name.ends_with(".gguf") => name.clone(),
            Err(_) => format!("{name}.gguf"),
        };
        let path = models_dir().join(&file);
        if path.is_file() {
            Ok(path)
        } else {
            Err(format!("No speech model installed ({file}). Run: ostt download"))
        }
    }

    pub fn engine_options(&self) -> Result<crate::engine::EngineOptions, String> {
        let language = Some(self.get("language")).filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case("auto"));
        Ok(crate::engine::EngineOptions {
            model: self.model_path()?,
            language,
            languages: crate::language_selection::parse_allowed(&self.get("languages")),
            translate: self.flag("translate"),
            threads: self.number("threads") as i32,
            device: self.get("device"),
            langid_model: langid_path(),
            terms_path: terms_path(),
            corrections_path: corrections_path(),
        })
    }
}
