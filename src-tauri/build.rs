fn main() {
    // Only run tauri_build when the gui feature is active.
    // Without this guard, `cargo test --lib --no-default-features` fails
    // because tauri-build requires Tauri-specific environment setup.
    #[cfg(feature = "gui")]
    tauri_build::build()
}
