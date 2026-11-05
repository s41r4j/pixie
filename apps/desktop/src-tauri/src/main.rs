// Prevents an extra console window on Windows in release; macOS is the target platform.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Chrome launches this same binary as the native messaging host for the extension.
    let args: Vec<String> = std::env::args().collect();
    if openclicky_native_host::is_native_host_launch(&args) {
        std::process::exit(openclicky_native_host::run(openclicky_native_host::default_socket()));
    }
    openclicky_desktop_lib::run()
}
