//! Standalone native host binary (the app binary also acts as the host; see lib.rs).
fn main() {
    let socket = std::env::var_os("PIXIE_BROWSER_SOCKET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(pixie_native_host::default_socket);
    std::process::exit(pixie_native_host::run(socket));
}
