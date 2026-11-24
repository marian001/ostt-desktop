This directory is egui-winit 0.35.0 from crates.io (https://github.com/emilk/egui, MIT OR Apache-2.0)
with one change in `src/lib.rs`: `modifiers.mac_cmd` is set from the Super key on every platform,
not only on macOS, so that a shortcut containing Super can be captured on Linux.
