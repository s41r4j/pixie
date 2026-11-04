// Prevents an extra console window on Windows in release; macOS is the target platform.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    openclicky_desktop_lib::run()
}
