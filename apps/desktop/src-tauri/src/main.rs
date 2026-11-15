// Prevents an extra console window on Windows in release; macOS is the target platform.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Chrome launches this same binary as the native messaging host for the extension.
    let args: Vec<String> = std::env::args().collect();
    if pixie_native_host::is_native_host_launch(&args) {
        std::process::exit(pixie_native_host::run(pixie_native_host::default_socket()));
    }
    // Pixie's speech engine runs as a child of this same program (see `pixie_stt::engine`).
    if let Some((runtime, model)) = pixie_stt::engine::is_engine_launch(&args) {
        std::process::exit(pixie_stt::engine::run(runtime, model));
    }
    pixie_lib::run()
}
