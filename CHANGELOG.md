# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **GUI Application**: Complete macOS Tauri application with WebView-based interface
- **Card Management UI**: Full-featured interface for creating, viewing, editing, and deleting flashcards
  - Cards tab: Browse all cards with detailed FSRS state information
  - Review tab: Single-card review mode with rating buttons  
  - Statistics tab: Dashboard showing card counts, state distribution, and averages for difficulty/stability
  - Settings tab: Configure notification channels (Integrated, Notification, Signal, Telegram)
- **Database Layer**: SQLite persistence with in-memory schema initialization
  - CRUD operations for cards
  - State conversion utilities for FSRS enum storage
  - Query optimization with indexed lookups
- **Tauri Command Handlers**: 11 commands for frontend-backend communication
  - `get_all_cards`: Retrieve all cards from database
  - `get_due_cards`: Retrieve cards due for review
  - `get_due_card`: Single card for review (returns Option)
  - `add_card`: Create new card with default FSRS values
  - `update_card`: Modify card content
  - `delete_card`: Remove card from database
  - `review_card`: Process review with rating (1-4)
  - `get_statistics`: Calculate and return card statistics
  - `get_settings`: Retrieve app settings
  - `update_settings`: Persist user settings
  - `show_review_window`: Create or focus review window
- **Comprehensive Test Suite**: 68 tests across 3 modules
  - **FSRS Module (31 tests)**: Algorithm correctness, stability/difficulty calculation, state transitions
  - **Database Module (21 tests)**: CRUD operations, state conversion, multi-card workflows, rating effects
  - **Main Module (16 tests)**: Data structure serialization, command logic, integration tests
- **Test Isolation**: In-memory SQLite databases for each test, preventing data conflicts
- **Apple Design System**: CSS styling matching macOS conventions
  - Native-looking buttons, modals, and layouts
  - Tab-based navigation
  - Form inputs and data grids
- **Tray Icon Support**: System tray integration (infrastructure ready)
- **Developer Tools**: Auto-enabled devtools in debug builds for console debugging

### Changed
- **Project Structure**: Migrated from CLI daemon to desktop application
  - CLI code preserved in `src-tauri/src/main_cli.rs` for reference
  - New main.rs with Tauri framework integration
  - Modular architecture: `db.rs`, `fsrs.rs`, `main.rs`
- **Build System**: 
  - Updated Rust to 1.93.1 (from 1.80.1)
  - Tauri 2.10.2 with plugin support
  - Icon generation with ImageMagick for multiple formats (PNG, ICNS, ICO)
- **Database Schema**: Enhanced with FSRS state persistence
  - Added `state` field for State enum (integer stored)
  - Added `scheduled_days` for next review interval
  - Added `lapses` counter for failed attempts
  - Added `due_at` timestamp for efficient queries
- **Frontend**: 
  - Migration from CLI to Tauri WebView
  - Vanilla JavaScript with Tauri API integration
  - HTML5 semantic markup
  - CSS Grid and Flexbox layouts

### Removed
- **Legacy CLI Features**: 
  - Command-line argument parsing
  - Environment variable configuration
  - Console-based card input/review
  - Signal and Telegram channel implementations (stubbed in settings)
- **Dependencies**:
  - Removed `clap` (CLI argument parsing)
  - Removed `tokio` (async runtime for daemon)
  - Removed notification crates (replaced by Tauri)

### Fixed
- **Configuration Issues**:
  - Fixed Tauri DevUrl configuration (removed, uses bundled assets)
  - Fixed global Tauri API injection (`withGlobalTauri: true`)
  - Fixed window URL configuration (`url: "index.html"`)
  - Fixed icon format to RGBA PNG (Tauri requirement)
- **Database Path**: Properly creates ~/Library/Application Support/teacha/ directory
- **Test Flakiness**: 
  - Fixed shared database state across tests with Database::test()
  - Eliminated test execution order dependencies
  - Ensured in-memory isolation per test

### Dependencies
- **Core**:
  - `rusqlite 0.32.1` - SQLite bindings
  - `chrono` - Timestamp handling
  - `serde_json` - JSON serialization
  - `tauri 2.10.2` - Desktop framework
- **Features**:
  - `tauri-plugin-shell` - Shell command execution
  - `tauri-plugin-dialog` - File dialogs (infrastructure)
- **Development**:
  - `cargo` - Rust build system
  - ImageMagick - Icon generation

## [0.1.0] - 2025-02-15

### Added
- Initial Rust CLI project setup
- FSRS (Free Spaced Repetition Scheduler) v5 implementation with 19 parameters
- 66 comprehensive unit tests for FSRS algorithm
- macOS notifications via `osascript`
- 90% retention target configuration
- Basic card state management (New, Learning, Review, Relearning)

### Notes
This version focused on establishing the core spaced repetition algorithm. The GUI and database layers came in subsequent updates to create a complete macOS application.

[Unreleased]: https://github.com/yourusername/teacha/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/teacha/releases/tag/v0.1.0
