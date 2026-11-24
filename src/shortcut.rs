//! The global dictation shortcut: validation against system and application shortcuts, and
//! storage in the COSMIC desktop's custom-shortcut file.
//!
//! The desktop owns global shortcuts, so the binding lives in its configuration and nowhere
//! else: what this module reads back is what the compositor will act on.

use std::path::PathBuf;

const SYSTEM_DEFAULTS: &str = "/usr/share/cosmic/com.system76.CosmicSettings.Shortcuts/v1/defaults";
const DESCRIPTION: &str = "Offline speech to text: dictation";

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Combo {
    pub superkey: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// XKB keysym name as the desktop stores it, e.g. `d`, `F9`, `space`.
    pub key: String,
}

/// Maps a key as the toolkit names it (`D`, `F9`, `Space`) to its XKB keysym name.
pub fn xkb_key(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let mut chars = name.chars();
    if let (Some(first), None) = (chars.next(), chars.next()) {
        if first.is_ascii_alphanumeric() {
            return Some(lower);
        }
    }
    if let Some(number) = lower.strip_prefix('f').and_then(|rest| rest.parse::<u32>().ok()) {
        if (1..=24).contains(&number) {
            return Some(format!("F{number}"));
        }
    }
    let mapped = match lower.as_str() {
        "space" => "space",
        "enter" | "return" => "Return",
        "escape" => "Escape",
        "tab" => "Tab",
        "backspace" => "BackSpace",
        "delete" => "Delete",
        "insert" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" | "prior" | "page_up" => "Prior",
        "pagedown" | "next" | "page_down" => "Next",
        "left" => "Left",
        "right" => "Right",
        "up" => "Up",
        "down" => "Down",
        "minus" => "minus",
        "equals" | "equal" => "equal",
        "comma" => "comma",
        "period" => "period",
        "slash" => "slash",
        "backslash" => "backslash",
        "semicolon" => "semicolon",
        "quote" | "apostrophe" => "apostrophe",
        "backtick" | "grave" => "grave",
        "openbracket" | "bracketleft" => "bracketleft",
        "closebracket" | "bracketright" => "bracketright",
        "print" => "Print",
        "pause" => "Pause",
        _ => return None,
    };
    Some(mapped.to_string())
}

impl Combo {
    /// Parses `Super+Alt+D`.
    pub fn parse(text: &str) -> Result<Combo, String> {
        let mut combo = Combo::default();
        for part in text.split('+').map(str::trim).filter(|part| !part.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "super" | "logo" | "meta" | "win" => combo.superkey = true,
                "ctrl" | "control" => combo.ctrl = true,
                "alt" => combo.alt = true,
                "shift" => combo.shift = true,
                _ if !combo.key.is_empty() => return Err(format!("'{text}' names more than one key")),
                _ => combo.key = xkb_key(part).ok_or_else(|| format!("'{part}' cannot be used in a shortcut"))?,
            }
        }
        if combo.key.is_empty() {
            return Err(format!("'{text}' has no key, only modifiers"));
        }
        Ok(combo)
    }

    fn function_key(&self) -> Option<u32> {
        self.key.strip_prefix('F').and_then(|rest| rest.parse().ok())
    }

    fn same_keys(&self, other: &Combo) -> bool {
        self.superkey == other.superkey
            && self.ctrl == other.ctrl
            && self.alt == other.alt
            && self.shift == other.shift
            && xkb_key(&self.key).map(|k| k.to_lowercase()) == xkb_key(&other.key).map(|k| k.to_lowercase())
    }
}

impl std::fmt::Display for Combo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (held, name) in [(self.superkey, "Super"), (self.ctrl, "Ctrl"), (self.alt, "Alt"), (self.shift, "Shift")] {
            if held {
                write!(f, "{name}+")?;
            }
        }
        let key = match self.key.as_str() {
            key if key.len() == 1 => key.to_ascii_uppercase(),
            "space" => "Space".into(),
            "Return" => "Enter".into(),
            "Prior" => "PageUp".into(),
            "Next" => "PageDown".into(),
            other => {
                let mut chars = other.chars();
                chars.next().map(|first| first.to_ascii_uppercase().to_string() + chars.as_str()).unwrap_or_default()
            }
        };
        write!(f, "{key}")
    }
}

/// What a well-known application shortcut does, for the refusal message.
fn application_meaning(combo: &Combo) -> Option<&'static str> {
    if combo.superkey || combo.alt {
        return None;
    }
    let meaning = match (combo.ctrl, combo.shift, combo.key.as_str()) {
        (true, false, "c") => "Copy",
        (true, false, "v") => "Paste",
        (true, false, "x") => "Cut",
        (true, false, "z") => "Undo",
        (true, false, "y") => "Redo",
        (true, false, "a") => "Select all",
        (true, false, "s") => "Save",
        (true, false, "f") => "Find",
        (true, false, "p") => "Print",
        (true, false, "n") => "New",
        (true, false, "o") => "Open",
        (true, false, "w") => "Close tab",
        (true, false, "t") => "New tab",
        (true, false, "q") => "Quit",
        (true, false, "r") => "Reload",
        (true, true, "z") => "Redo",
        (true, true, "c") => "Copy in terminals",
        (true, true, "v") => "Paste in terminals",
        (true, true, "t") => "Reopen closed tab",
        _ => return None,
    };
    Some(meaning)
}

/// Refuses a combination that the system or applications already use, with the reason.
///
/// `taken` are the desktop's own shortcuts (other than ours) with what each one does.
pub fn check(combo: &Combo, taken: &[(Combo, String)]) -> Result<(), String> {
    if combo.ctrl && combo.alt && !combo.superkey {
        if matches!(combo.key.as_str(), "Delete" | "BackSpace") {
            return Err(format!("{combo} is reserved by the system."));
        }
        if combo.function_key().is_some_and(|number| number <= 12) {
            return Err(format!("{combo} switches to a text console."));
        }
    }
    if let Some((_, action)) = taken.iter().find(|(other, _)| other.same_keys(combo)) {
        return Err(format!("{combo} is already a desktop shortcut ({action})."));
    }
    let global = combo.superkey || (combo.ctrl && combo.alt) || combo.function_key().is_some_and(|number| number >= 13);
    if global {
        return Ok(());
    }
    const ADVICE: &str = "Add Super, or use Ctrl+Alt.";
    if let Some(meaning) = application_meaning(combo) {
        return Err(format!("{combo} is used by applications ({meaning}). {ADVICE}"));
    }
    if !combo.ctrl && !combo.alt {
        return Err(format!("{combo} would fire while typing. {ADVICE}"));
    }
    Err(format!("{combo} is left to applications (menus, editing, tabs). {ADVICE}"))
}

/// Splits the body of a COSMIC shortcut file into its top-level `(binding): Action` entries.
fn entries(raw: &str) -> Vec<String> {
    let (Some(open), Some(close)) = (raw.find('{'), raw.rfind('}')) else { return Vec::new() };
    if close <= open {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut current = String::new();
    let (mut depth, mut in_string, mut escaped) = (0i32, false, false);
    for ch in raw[open + 1..close].chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                if !current.trim().is_empty() {
                    result.push(current.trim().to_string());
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }
    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }
    result
}

/// Splits one entry into its binding and its action at the `):` that closes the binding.
fn split_entry(entry: &str) -> Option<(&str, &str)> {
    let (mut depth, mut in_string, mut escaped) = (0i32, false, false);
    for (index, ch) in entry.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let action = entry[index + 1..].trim_start().strip_prefix(':')?;
                    return Some((&entry[..=index], action.trim()));
                }
            }
            _ => {}
        }
    }
    None
}

fn binding_combo(binding: &str) -> Option<Combo> {
    let list_start = binding.find("modifiers:")? + "modifiers:".len();
    let list = &binding[list_start..];
    let list = &list[list.find('[')? + 1..list.find(']')?];
    let mut combo = Combo::default();
    for modifier in list.split(',').map(str::trim) {
        match modifier {
            "Super" => combo.superkey = true,
            "Ctrl" => combo.ctrl = true,
            "Alt" => combo.alt = true,
            "Shift" => combo.shift = true,
            _ => {}
        }
    }
    let after = &binding[binding.find("key:")? + "key:".len()..];
    let after = &after[after.find('"')? + 1..];
    combo.key = after[..after.find('"')?].to_string();
    Some(combo)
}

fn is_ours(action: &str) -> bool {
    action.starts_with("Spawn(") && action.contains("ostt toggle\"")
}

fn parse_file(raw: &str) -> Vec<(Combo, String, bool)> {
    entries(raw)
        .iter()
        .filter_map(|entry| {
            let (binding, action) = split_entry(entry)?;
            Some((binding_combo(binding)?, action.to_string(), is_ours(action)))
        })
        .collect()
}

fn custom_path() -> PathBuf {
    match std::env::var_os("OSTT_SHORTCUTS_FILE") {
        Some(path) => PathBuf::from(path),
        None => crate::config::config_dir()
            .parent()
            .map(|config| config.join("cosmic/com.system76.CosmicSettings.Shortcuts/v1/custom"))
            .unwrap_or_default(),
    }
}

/// Only COSMIC is handled; elsewhere the user binds `ostt toggle` in the desktop's own settings.
pub fn supported() -> bool {
    std::env::var_os("OSTT_SHORTCUTS_FILE").is_some()
        || (std::env::var("XDG_CURRENT_DESKTOP").map(|d| d.to_uppercase().contains("COSMIC")).unwrap_or(false)
            || custom_path().parent().is_some_and(|dir| dir.is_dir()))
}

/// The shortcut currently bound to dictation, if any.
pub fn current() -> Option<Combo> {
    let raw = std::fs::read_to_string(custom_path()).ok()?;
    parse_file(&raw).into_iter().find(|(_, _, ours)| *ours).map(|(combo, _, _)| combo)
}

/// Desktop shortcuts other than ours: the user's custom ones and the system defaults.
pub fn taken() -> Vec<(Combo, String)> {
    let mut all = Vec::new();
    for path in [custom_path(), PathBuf::from(SYSTEM_DEFAULTS)] {
        if let Ok(raw) = std::fs::read_to_string(path) {
            all.extend(parse_file(&raw).into_iter().filter(|(_, _, ours)| !ours).map(|(combo, action, _)| (combo, action)));
        }
    }
    all
}

fn rewrite(new_entry: Option<String>) -> Result<(), String> {
    let path = custom_path();
    let raw = std::fs::read_to_string(&path).unwrap_or_default();
    // Keep one copy of the file as it was before this program first touched it.
    let backup = crate::config::state_dir().join("cosmic-shortcuts-custom.before-ostt");
    if !raw.is_empty() && !backup.exists() {
        let _ = std::fs::create_dir_all(crate::config::state_dir());
        let _ = std::fs::write(&backup, &raw);
    }
    // Every other entry is written back exactly as it was found.
    let mut kept: Vec<String> = entries(&raw)
        .into_iter()
        .filter(|entry| !split_entry(entry).is_some_and(|(_, action)| is_ours(action)))
        .collect();
    kept.extend(new_entry);
    let mut out = String::from("{\n");
    for entry in &kept {
        out.push_str("    ");
        out.push_str(entry);
        out.push_str(",\n");
    }
    out.push('}');
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("ostt-tmp");
    std::fs::write(&tmp, out).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// Binds dictation to `combo`, replacing any earlier binding, after checking for collisions.
pub fn set(combo: &Combo) -> Result<(), String> {
    check(combo, &taken())?;
    let mut modifiers = Vec::new();
    for (held, name) in [(combo.superkey, "Super"), (combo.shift, "Shift"), (combo.ctrl, "Ctrl"), (combo.alt, "Alt")] {
        if held {
            modifiers.push(name);
        }
    }
    let exe = crate::config::config_dir()
        .parent()
        .and_then(|config| config.parent())
        .map(|home| home.join(".local/bin/ostt"))
        .filter(|path| path.exists())
        .or_else(|| std::env::current_exe().ok())
        .ok_or("cannot locate the ostt program")?;
    let entry = format!(
        "(\n        modifiers: [{}],\n        key: \"{}\",\n        description: Some(\"{DESCRIPTION}\"),\n    ): Spawn(\"{} toggle\")",
        modifiers.join(", "),
        combo.key,
        exe.display()
    );
    rewrite(Some(entry))
}

/// Removes the dictation shortcut; dictation stays available from the tray icon.
pub fn clear() -> Result<(), String> {
    if current().is_none() {
        return Ok(());
    }
    rewrite(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(text: &str) -> Combo {
        Combo::parse(text).unwrap()
    }

    const DEFAULTS: &str = r#"{
    (modifiers: [Super], key: "t"): System(Terminal),
    (modifiers: [Alt], key: "F4"): Close,
    (modifiers: [Super, Shift], key: "r"): Resizing(Inwards),
    (modifiers: [Super]): System(Launcher),
    (modifiers: [], key: "Print"): System(Screenshot),
}"#;

    fn taken_defaults() -> Vec<(Combo, String)> {
        parse_file(DEFAULTS).into_iter().map(|(combo, action, _)| (combo, action)).collect()
    }

    #[test]
    fn parses_and_prints_combinations() {
        let parsed = combo("super+alt+d");
        assert_eq!(parsed, Combo { superkey: true, alt: true, key: "d".into(), ..Default::default() });
        assert_eq!(parsed.to_string(), "Super+Alt+D");
        assert_eq!(combo("Ctrl+Alt+Space").to_string(), "Ctrl+Alt+Space");
        assert_eq!(combo("F13").key, "F13");
        assert!(Combo::parse("Super+Alt").is_err());
        assert!(Combo::parse("Super+A+B").is_err());
    }

    #[test]
    fn application_shortcuts_are_refused_with_their_meaning() {
        let taken = taken_defaults();
        let error = check(&combo("Ctrl+C"), &taken).unwrap_err();
        assert!(error.contains("Copy"), "{error}");
        assert!(check(&combo("Ctrl+Shift+V"), &taken).is_err());
        assert!(check(&combo("Ctrl+K"), &taken).is_err());
        assert!(check(&combo("Alt+D"), &taken).is_err());
        assert!(check(&combo("Shift+D"), &taken).unwrap_err().contains("typing"));
        assert!(check(&combo("D"), &taken).is_err());
        assert!(check(&combo("F5"), &taken).is_err());
    }

    #[test]
    fn desktop_shortcuts_are_refused_and_free_ones_accepted() {
        let taken = taken_defaults();
        assert!(check(&combo("Super+T"), &taken).unwrap_err().contains("System(Terminal)"));
        assert!(check(&combo("Alt+F4"), &taken).is_err());
        assert!(check(&combo("Super+Shift+R"), &taken).is_err());
        assert!(check(&combo("Ctrl+Alt+Delete"), &taken).is_err());
        assert!(check(&combo("Ctrl+Alt+F2"), &taken).is_err());
        assert!(check(&combo("Super+Alt+D"), &taken).is_ok());
        assert!(check(&combo("Ctrl+Alt+D"), &taken).is_ok());
        assert!(check(&combo("Super+F9"), &taken).is_ok());
        assert!(check(&combo("F13"), &taken).is_ok());
    }

    #[test]
    fn other_entries_survive_a_rewrite_untouched() {
        let existing = "{\n    (\n        modifiers: [],\n        key: \"XF86Calculator\",\n        description: Some(\"Launch, (Calc)\"),\n    ): Spawn(\"a && b, c\"),\n}";
        let found = entries(existing);
        assert_eq!(found.len(), 1);
        let (binding, action) = split_entry(&found[0]).unwrap();
        assert_eq!(binding_combo(binding).unwrap().key, "XF86Calculator");
        assert_eq!(action, "Spawn(\"a && b, c\")");
        assert!(!is_ours(action));
        assert!(is_ours("Spawn(\"/home/u/.local/bin/ostt toggle\")"));
    }
}
