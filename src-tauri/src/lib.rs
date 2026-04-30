// Core library: db and fsrs are independent of Tauri.
// Exposing them here lets `cargo test --lib` compile and run all tests
// without requiring WebKitGTK or any other GUI system library.
pub mod db;
pub mod fsrs;
pub mod seed;
