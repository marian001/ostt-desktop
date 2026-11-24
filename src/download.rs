//! Resumable model downloads that are activated only after their exact size and SHA-256 match.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use std::process::Command;

use crate::catalog::Entry;

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

fn verify(path: &Path, entry: &Entry) -> Result<(), String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size != entry.bytes {
        return Err(format!("size is {size} bytes, expected {}", entry.bytes));
    }
    let hash = sha256_file(path)?;
    if hash != entry.sha256 {
        return Err(format!("SHA-256 is {hash}, expected {}", entry.sha256));
    }
    Ok(())
}

/// Downloads `entry` into `dir`. An interrupted download resumes; a file that fails
/// verification is removed and never replaces an installed model.
pub fn fetch(entry: &Entry, dir: &Path) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let target = dir.join(entry.file);
    if target.is_file() {
        if verify(&target, entry).is_ok() {
            eprintln!("{} is already installed and verified.", entry.file);
            return Ok(target);
        }
        eprintln!("{} is installed but does not match the catalogue; downloading again.", entry.file);
    }
    let part = dir.join(format!("{}.part", entry.file));
    if std::fs::metadata(&part).map(|m| m.len() > entry.bytes).unwrap_or(false) {
        let _ = std::fs::remove_file(&part);
    }
    eprintln!("Downloading {} ({:.0} MB) from huggingface.co/handy-computer/{}", entry.file, entry.bytes as f64 / 1e6, entry.repo);
    let complete = std::fs::metadata(&part).map(|m| m.len() == entry.bytes).unwrap_or(false);
    if !complete {
        let status = Command::new("curl")
            .args(["--fail", "--location", "--continue-at", "-", if std::env::var_os("OSTT_QUIET_DOWNLOAD").is_some() { "--silent" } else { "--progress-bar" }, "--retry", "3", "--output"])
            .arg(&part)
            .arg(entry.url())
            .status()
            .map_err(|e| format!("cannot run curl: {e}"))?;
        if !status.success() {
            return Err("download failed; run the same command again to resume".into());
        }
    }
    eprintln!("Verifying SHA-256...");
    if let Err(reason) = verify(&part, entry) {
        let _ = std::fs::remove_file(&part);
        return Err(format!("downloaded file rejected: {reason}"));
    }
    std::fs::rename(&part, &target).map_err(|e| e.to_string())?;
    eprintln!("Installed {}", target.display());
    Ok(target)
}
