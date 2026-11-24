//! ostt — offline speech to text for the Linux desktop.
//!
//! A desktop port of Offline Speech to Text for Android (MIT, © 2025 Noah Mühl). The modules
//! under `core/` are taken from that project unchanged; everything else here replaces its
//! Android layer: PipeWire capture, a background service with a tray icon, and typing into
//! the focused window.

#![allow(dead_code)]

#[path = "core/audio.rs"]
mod audio;
#[path = "core/audio_finalization.rs"]
mod audio_finalization;
#[path = "core/custom_terms.rs"]
mod custom_terms;
#[path = "core/endpoint.rs"]
mod endpoint;
#[path = "core/english_fillers.rs"]
mod english_fillers;
#[path = "core/interactive_endpointing.rs"]
mod interactive_endpointing;
#[path = "core/language_selection.rs"]
mod language_selection;
#[path = "core/long_audio.rs"]
mod long_audio;
#[path = "core/personal_corrections.rs"]
mod personal_corrections;
#[path = "core/preferred_terms.rs"]
mod preferred_terms;
#[path = "core/transcript_cleanup.rs"]
mod transcript_cleanup;
#[path = "core/transcript_guard.rs"]
mod transcript_guard;

mod audio_io;
mod catalog;
mod config;
mod daemon;
mod download;
mod engine;
mod gui;
mod inject;
mod shortcut;
mod tray;

use config::Config;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

const USAGE: &str = "\
ostt — offline speech to text

Dictation (needs the service running):
  ostt daemon                 run the background service (model stays loaded, tray icon)
  ostt toggle                 start recording, or finish and insert the text   <- bind to a key
  ostt start | stop | cancel  the same, as separate steps
  ostt status                 idle | recording | transcribing | loading | error
  ostt copy-last              put the last transcript on the clipboard
  ostt reload                 reload the model and settings
  ostt quit                   stop the service

Without the service:
  ostt file <audio> [...]     transcribe audio files to standard output
  ostt record [seconds]       record the microphone (Enter to finish) and print the text

Setup:
  ostt gui                    open the settings window
  ostt models                 list the model catalogue and what is installed
  ostt download [id|langid]   download and verify a model (default: Parakeet TDT 0.6B v3 Q4)
  ostt use <id>               select an installed model
  ostt devices                list compute devices
  ostt config                 show settings
  ostt config set <key> <v>   change a setting
  ostt shortcut [set <combo> | clear]   show, set (e.g. Super+Alt+D) or remove the dictation shortcut
  ostt bench <audio>          time the selected model on every device
";

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("ostt: {}", message.as_ref());
    std::process::exit(1)
}

fn quiet_native_logs() {
    if std::env::var_os("OSTT_DEBUG").is_none() {
        transcribe_cpp::disable_logging();
    }
}

fn client(command: &str) {
    match daemon::request(command) {
        Ok(reply) => {
            println!("{reply}");
            if reply.starts_with("error") {
                std::process::exit(1);
            }
        }
        Err(_) => fail("the service is not running — start it with `ostt daemon` (or: systemctl --user start ostt)"),
    }
}

fn load_engine(config: &Config) -> engine::Engine {
    let options = config.engine_options().unwrap_or_else(|e| fail(e));
    engine::Engine::load(&options).unwrap_or_else(|e| fail(e))
}

fn cmd_file(paths: &[String]) {
    if paths.is_empty() {
        fail("usage: ostt file <audio> [...]");
    }
    let mut engine = load_engine(&Config::load());
    for path in paths {
        let samples = audio_io::read_file(Path::new(path)).unwrap_or_else(|e| fail(e));
        let started = Instant::now();
        let text = engine.transcribe(&samples).unwrap_or_else(|e| fail(e));
        eprintln!(
            "ostt: {path}: {:.1}s of audio in {:.2}s on {}",
            samples.len() as f64 / audio_io::SAMPLE_RATE as f64,
            started.elapsed().as_secs_f64(),
            engine.backend
        );
        println!("{text}");
    }
}

fn cmd_record(seconds: Option<f64>) {
    let config = Config::load();
    let mut engine = load_engine(&config);
    let recorder = audio_io::Recorder::start(&config.get("source")).unwrap_or_else(|e| fail(e));
    match seconds {
        Some(seconds) => {
            eprintln!("ostt: recording for {seconds} s...");
            std::thread::sleep(std::time::Duration::from_secs_f64(seconds));
        }
        None => {
            eprintln!("ostt: recording — press Enter to finish.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
    }
    let samples = recorder.finish().unwrap_or_else(|e| fail(e));
    let text = engine.transcribe(&samples).unwrap_or_else(|e| fail(e));
    println!("{text}");
}

fn installed_files() -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(config::models_dir())
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".gguf"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

fn cmd_models() {
    let installed = installed_files();
    let active = Config::load().model_path().ok();
    let active = active.as_deref().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned());
    let mut family = "";
    for entry in catalog::MODELS {
        if entry.family != family {
            family = entry.family;
            println!("\n{family}");
        }
        let mark = if active.as_deref() == Some(entry.file) {
            "*"
        } else if installed.iter().any(|name| name == entry.file) {
            "+"
        } else {
            " "
        };
        println!("  {mark} {:<44} {:>6.0} MB", entry.id(), entry.bytes as f64 / 1e6);
    }
    for file in installed.iter().filter(|name| !catalog::MODELS.iter().any(|e| e.file == **name)) {
        let mark = if active.as_deref() == Some(file.as_str()) { "*" } else { "+" };
        println!("\nImported\n  {mark} {file}");
    }
    let detector = if config::langid_path().is_file() { "installed" } else { "not installed" };
    println!("\n* active   + installed   language detector (`ostt download langid`): {detector}");
}

fn cmd_download(target: Option<&String>) {
    let result = match target.map(String::as_str) {
        Some("langid") => {
            let dir = config::langid_path().parent().map(Path::to_path_buf).unwrap_or_default();
            download::fetch(&catalog::LANGUAGE_ID, &dir).map(|_| ())
        }
        other => {
            let entry = catalog::find(other.unwrap_or(catalog::DEFAULT_FILE)).unwrap_or_else(|e| fail(e));
            download::fetch(entry, &config::models_dir()).map(|_| {
                if Config::load().get("model").is_empty() && entry.file != catalog::DEFAULT_FILE {
                    eprintln!("Select it with: ostt use {}", entry.id());
                }
            })
        }
    };
    if let Err(error) = result {
        fail(error);
    }
}

fn cmd_use(target: Option<&String>) {
    let Some(target) = target else { fail("usage: ostt use <id>") };
    let value = match catalog::find(target) {
        Ok(entry) => entry.file.to_string(),
        Err(_) if Path::new(target).is_file() => std::fs::canonicalize(target)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|e| fail(e.to_string())),
        Err(error) => fail(error),
    };
    let previous = Config::load().get("model");
    Config::set("model", &value).unwrap_or_else(|e| fail(e));
    if let Err(error) = Config::load().model_path() {
        // Never leave the configuration pointing at a model that is not there.
        let _ = Config::set("model", &previous);
        fail(error);
    }
    println!("model = {value}");
    if daemon::request("reload").is_ok() {
        println!("service reloading");
    }
}

fn cmd_devices() {
    let (_, auto) = engine::pick_device("auto").unwrap_or((transcribe_cpp::Backend::Cpu, None));
    let auto = auto.map(|device| device.name);
    for device in transcribe_cpp::devices() {
        let chosen = match &auto {
            Some(name) => *name == device.name,
            None => device.device_type == transcribe_cpp::DeviceType::Cpu,
        };
        println!(
            "{} {:<9} {:<5} {:<45} {:>6.1} GB",
            if chosen { "*" } else { " " },
            device.name,
            format!("{:?}", device.device_type).to_lowercase(),
            device.description,
            device.memory_total as f64 / 1e9
        );
    }
    println!("\n* = what `device = auto` picks. Set another with: ostt config set device <name>");
}

fn cmd_config(args: &[String]) {
    match args.first().map(String::as_str) {
        Some("set") => {
            let (Some(key), Some(value)) = (args.get(1), args.get(2)) else {
                fail("usage: ostt config set <key> <value>")
            };
            Config::set(key, value).unwrap_or_else(|e| fail(e));
            println!("{key} = {value}");
            if matches!(key.as_str(), "model" | "device" | "language" | "languages" | "translate" | "threads")
                && daemon::request("reload").is_ok()
            {
                println!("service reloading");
            }
        }
        Some("path") => println!("{}", config::config_path().display()),
        None => {
            let config = Config::load();
            for (key, _, help) in config::KEYS {
                println!("{:<14} = {:<34} # {help}", key, config.get(key));
            }
            println!("\nfile:         {}", config::config_path().display());
            println!("dictionary:   {}   (one preferred word or phrase per line)", config::terms_path().display());
            println!("corrections:  {}   (heard => written, one per line)", config::corrections_path().display());
            println!("models:       {}", config::models_dir().display());
        }
        Some(other) => fail(format!("unknown: ostt config {other}")),
    }
}

fn cmd_bench(path: Option<&String>) {
    let Some(path) = path else { fail("usage: ostt bench <audio>") };
    let samples = audio_io::read_file(Path::new(path)).unwrap_or_else(|e| fail(e));
    let seconds = samples.len() as f64 / audio_io::SAMPLE_RATE as f64;
    let config = Config::load();
    let names: Vec<String> = transcribe_cpp::devices().into_iter().map(|device| device.name).collect();
    println!("{:.1}s of audio, {}", seconds, config.model_path().map(|p| p.display().to_string()).unwrap_or_default());
    for name in names {
        let mut options = config.engine_options().unwrap_or_else(|e| fail(e));
        options.device = name.clone();
        let started = Instant::now();
        let mut engine = match engine::Engine::load(&options) {
            Ok(engine) => engine,
            Err(error) => {
                println!("{name:<9} failed: {error}");
                continue;
            }
        };
        let load = started.elapsed().as_secs_f64();
        let mut best = f64::MAX;
        for _ in 0..3 {
            let started = Instant::now();
            if engine.transcribe(&samples).is_err() {
                break;
            }
            best = best.min(started.elapsed().as_secs_f64());
        }
        println!("{name:<9} load+warm-up {load:>5.2}s   best of 3: {best:>6.3}s   ({:.0}x real time)", seconds / best);
        let _ = std::io::stdout().flush();
    }
}

fn main() {
    // Let `ostt models | head` end quietly instead of panicking on a closed pipe.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let rest = if args.is_empty() { &args[..] } else { &args[1..] };
    match command {
        "daemon" => {
            quiet_native_logs();
            if let Err(error) = daemon::run() {
                fail(error);
            }
        }
        "gui" | "settings" => {
            quiet_native_logs();
            if let Err(error) = gui::run() {
                fail(error);
            }
        }
        "toggle" | "start" | "stop" | "cancel" | "status" | "reload" | "quit" | "copy-last" => client(command),
        "file" => {
            quiet_native_logs();
            cmd_file(rest)
        }
        "record" => {
            quiet_native_logs();
            cmd_record(rest.first().and_then(|value| value.parse().ok()))
        }
        "models" => cmd_models(),
        "download" => cmd_download(rest.first()),
        "use" => cmd_use(rest.first()),
        "devices" => {
            quiet_native_logs();
            cmd_devices()
        }
        "config" => cmd_config(rest),
        "shortcut" => match (rest.first().map(String::as_str), rest.get(1)) {
            (None, _) => match shortcut::current() {
                Some(combo) => println!("{combo}"),
                None => println!("none"),
            },
            (Some("clear"), _) => {
                shortcut::clear().unwrap_or_else(|e| fail(e));
                println!("none");
            }
            (Some("set"), Some(text)) => {
                let combo = shortcut::Combo::parse(text).unwrap_or_else(|e| fail(e));
                shortcut::set(&combo).unwrap_or_else(|e| fail(e));
                println!("{combo}");
            }
            _ => fail("usage: ostt shortcut [set <combo> | clear]"),
        },
        "bench" => {
            quiet_native_logs();
            cmd_bench(rest.first())
        }
        "version" | "--version" | "-V" => {
            println!("ostt {} (transcribe.cpp {})", env!("CARGO_PKG_VERSION"), transcribe_cpp::version())
        }
        "help" | "--help" | "-h" => print!("{USAGE}"),
        other => {
            eprint!("ostt: unknown command '{other}'\n\n{USAGE}");
            std::process::exit(2);
        }
    }
}
