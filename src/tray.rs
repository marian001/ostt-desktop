//! Status icon in the panel (StatusNotifierItem): click to start or finish a dictation.

use std::sync::Arc;

use crate::daemon::Daemon;

pub struct OsttTray {
    daemon: Arc<Daemon>,
    pub status: &'static str,
    pub detail: String,
}

impl OsttTray {
    /// Tray callbacks run on the tray's own thread, and the service updates the tray from its
    /// actions; running an action inline would wait on itself.
    fn act(&self, action: fn(&Arc<Daemon>) -> String) {
        let daemon = self.daemon.clone();
        std::thread::spawn(move || {
            action(&daemon);
        });
    }
}

impl ksni::Tray for OsttTray {
    fn id(&self) -> String {
        "ostt".into()
    }

    fn title(&self) -> String {
        format!("OSTT — {}", self.status)
    }

    fn icon_name(&self) -> String {
        match self.status {
            "recording" => "media-record-symbolic",
            "transcribing" | "loading" => "emblem-synchronizing-symbolic",
            "error" => "dialog-warning-symbolic",
            _ => "audio-input-microphone-symbolic",
        }
        .into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: format!("Offline speech to text — {}", self.status),
            description: self.detail.clone(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.act(|daemon| daemon.toggle());
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let recording = self.status == "recording";
        vec![
            StandardItem {
                label: if recording { "Finish dictation".into() } else { "Start dictation".into() },
                enabled: self.status != "transcribing",
                activate: Box::new(|tray: &mut Self| tray.act(|daemon| daemon.toggle())),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Discard recording".into(),
                enabled: recording,
                activate: Box::new(|tray: &mut Self| tray.act(|daemon| daemon.cancel())),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Copy last transcript".into(),
                activate: Box::new(|tray: &mut Self| tray.act(|daemon| daemon.copy_last())),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: if self.detail.is_empty() { self.status.to_string() } else { self.detail.clone() },
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Settings…".into(),
                activate: Box::new(|_| crate::gui::spawn_window()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Reload model and settings".into(),
                activate: Box::new(|tray: &mut Self| {
                    tray.act(|daemon| {
                        daemon.load_engine();
                        String::new()
                    })
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|_| {
                    let _ = crate::daemon::request("quit");
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn(daemon: Arc<Daemon>) -> Result<ksni::blocking::Handle<OsttTray>, String> {
    use ksni::blocking::TrayMethods;
    OsttTray { daemon, status: "loading", detail: String::new() }
        .spawn()
        .map_err(|e| e.to_string())
}
