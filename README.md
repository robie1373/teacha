# teacha

A macOS spaced repetition flashcard application with desktop GUI, built with Tauri, Rust, and FSRS v5 algorithm.

## Quick Start

### Prerequisites
- Rust 1.93.1+ (install via [rustup](https://rustup.rs/))
- macOS 10.15+ with Xcode Command Line Tools
- Tauri CLI: `cargo install tauri-cli`

### Build and Run

```bash
# Debug mode (includes developer tools)
cd src-tauri
cargo tauri dev

# Release mode
cargo tauri build
```

The app will launch with a database at `~/Library/Application Support/teacha/cards.db`.

## Features

### Card Management
- **Create Cards**: Add flashcards with prompt and answer
- **Browse Cards**: View all cards with FSRS state information
- **Edit Cards**: Update card content
- **Delete Cards**: Remove cards from database
- **Review Interface**: Dedicated review window with rating buttons

### Spaced Repetition
- **FSRS v5 Algorithm**: 19-parameter weights, 90% retention target
- **Intelligent Scheduling**: Automatic interval calculation based on card difficulty and performance
- **State Tracking**: New → Learning → Review → Relearning progression
- **Stability & Difficulty**: Dynamic parameters that improve with each review

### Statistics Dashboard
- **Card Counts**: Total, due, new, learning, and review state breakdowns
- **Averages**: Mean difficulty and stability across reviewed cards
- **Visual Overview**: Dashboard view of learning progress

### Settings
- **Notification Channels**: Configure notification delivery (Integrated, Notification, Signal, Telegram)
- **Poll Interval**: Customize review reminder frequency (default: 60 seconds)
- **Channel Selection**: Choose which channels receive notifications

## Architecture

### Project Structure

```
teacha/
├── src-tauri/              # Tauri application
│   ├── src/
│   │   ├── main.rs         # App entry point, Tauri commands
│   │   ├── db.rs           # SQLite database layer
│   │   ├── fsrs.rs         # FSRS algorithm implementation
│   │   └── main_cli.rs     # Legacy CLI (preserved for reference)
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri configuration
├── src/                    # Frontend (WebView)
│   ├── index.html          # Main UI
│   ├── review.html         # Review window UI
│   ├── app.js              # Frontend logic
│   └── styles.css          # Apple design system styling
├── icons/                  # Application icons
├── CHANGELOG.md            # Version history
└── README.md              # This file
```

### Technology Stack

**Backend:**
- **Rust** - Type-safe systems programming language
- **Tauri 2.10.2** - Lightweight desktop framework
- **SQLite** - Lightweight relational database
- **FSRS v5** - Algorithmic implementation of spaced repetition

**Frontend:**
- **HTML5** - Semantic markup
- **Vanilla JavaScript** - Tauri API integration
- **CSS3** - Grid, Flexbox, Apple design conventions
- **Tauri API** - Backend communication via `invoke()`

## Testing

All code is covered by 68 comprehensive tests across three modules:

```bash
cd src-tauri
cargo test          # Run all tests (68 tests)
cargo test --lib   # Library tests only
cargo test fsrs    # FSRS algorithm tests
cargo test db      # Database tests
cargo test tests   # Main/integration tests
```

### Test Coverage
- **FSRS Module (31 tests)**: Algorithm correctness, stability calculations, state transitions
- **Database Module (21 tests)**: CRUD operations, card reviews, multi-card interactions
- **Main Module (16 tests)**: Data serialization, command handlers, integration workflows

Each test runs in isolation with an in-memory SQLite database.

## Database Schema

```sql
CREATE TABLE cards (
  id INTEGER PRIMARY KEY,
  prompt TEXT NOT NULL,
  answer TEXT NOT NULL,
  stability REAL DEFAULT 0.0,
  difficulty REAL DEFAULT 0.0,
  elapsed_days REAL DEFAULT 0.0,
  scheduled_days REAL DEFAULT 0.0,
  reps INTEGER DEFAULT 0,
  lapses INTEGER DEFAULT 0,
  state INTEGER DEFAULT 0,  -- 0: New, 1: Learning, 2: Review, 3: Relearning
  last_review REAL DEFAULT 0.0,
  due_at INTEGER NOT NULL,    -- Unix timestamp
  created_at INTEGER NOT NULL
);
```

## Available Tauri Commands

### Card Operations
- `get_all_cards()` → `Vec<Card>` - Get all cards
- `get_due_cards()` → `Vec<Card>` - Get cards ready for review
- `get_due_card()` → `Option<Card>` - Get single card for review
- `add_card(prompt, answer)` → `i64` - Create new card (returns ID)
- `update_card(id, prompt, answer)` → `()` - Update card content
- `delete_card(id)` → `()` - Delete card
- `review_card(id, rating)` → `Card` - Submit review (rating: 1-4)

### Statistics & Settings
- `get_statistics()` → `Statistics` - Get dashboard statistics
- `get_settings()` → `JSON` - Get current settings
- `update_settings(channel, signal, telegram)` → `()` - Update settings
- `show_review_window()` → `()` - Create/focus review window

## File Locations

- **Database**: `~/Library/Application Support/teacha/cards.db`
- **Application**: `/Applications/Teacha.app` (after release build)
- **Logs**: Console.app with Tauri debugging

## Development Notes

### Building for Release
```bash
cd src-tauri
cargo tauri build --release
```

Outputs to `src-tauri/target/release/` including:
- `.app` bundle (macOS application)
- `.dmg` disk image  
- Signed executable (if code signing configured)

### Debugging
- **Frontend**: Press Cmd+R to reload UI
- **Console**: Cmd+Shift+I to open developer tools (debug mode only)
- **Database**: Direct SQLite3 access via `sqlite3 ~/Library/Application\ Support/teacha/cards.db`

### Icon Generation
Icons are generated via ImageMagick. To regenerate:
```bash
cd icons
chmod +x gen.sh
./gen.sh
```

Requires 32x32, 128x128, 128x128@2x PNG files and generates:
- `icon.icns` (macOS)
- `icon.ico` (Windows)
- `icon.png` (Linux)

## Roadmap

### Planned Features
- [ ] Keyboard shortcuts for navigation and review
- [ ] Dark mode support
- [ ] Card tags/categories
- [ ] Import/export (Anki format)
- [ ] Cloud sync
- [ ] Multi-user sync via database URLs
- [ ] Signal and Telegram notification implementation
- [ ] macOS menu bar integration
- [ ] Pkg installer for distribution
- [ ] Analytics dashboard

### Known Limitations
- Signal and Telegram channels are stubbed (settings UI ready)
- No cloud backup (local database only)
- Single-device only (no sync)
- Review window not yet fully integrated

## Contributing

Pull requests welcome! Areas of focus:
- Frontend UI improvements
- Channel implementations (Signal, Telegram)
- Performance optimization
- macOS-specific features

## License

MIT

## Credits

- **FSRS Algorithm**: [Open Spaced Repetition](https://github.com/open-spaced-repetition/fsrs4anki)
- **Tauri**: [Tauri Studio](https://tauri.app)
- **Rust**: [The Rust Foundation](https://foundation.rust-lang.org/)


[ ] Add support for additional notification channels (e.g., email, SMS). 
[ ] Create a user-friendly interface for managing cards and reviewing schedules. 
[x] Write unit tests to ensure the reliability of the application.
[ ] update .gitignore.
[ ] update test suite to cover entire app and remove legacy tests that no longer apply.
[ ] update README with usage instructions, configuration options, and development notes.
[ ] add a change log to track updates and improvements to the project.
[ ] find a more elegant way to handle the integrated window. maybe clicking on the mac notificaation could load the integrated window so a rating can be given.
[ ] fix the bug where the review card window shows loading... instead of the card content.
[ ] put a release together and publish the app to github releases.
[ ] build releases for windows and linux as well, and update the README with instructions for those platforms. use conditionals to ensure sensible notification options are the default on each platform. for example, on windows the default notification channel could be the Windows Notification Service, and on linux it could be libnotify or a custom solution depending on the desktop environment.
[ ] add ui elements to delete cards.
[ ] display state information in the cards tab and on the cards.

