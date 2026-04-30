# Teacha

Ambient spaced repetition for CLI knowledge. Teacha runs in the background and periodically fires desktop or push notifications reminding you of commands, shortcuts, and tools you keep forgetting. Cards are scheduled using [FSRS v5](https://github.com/open-spaced-repetition/fsrs4anki/wiki/The-Algorithm) — the same algorithm used in Anki — so things you know well show up rarely, and things you're shaky on show up often.

It's not a study app. You don't sit down with Teacha. It interrupts you with a two-second reminder while you're doing something else, and over weeks the knowledge sticks.

---

## How it works

1. A daemon polls a local SQLite database for cards that are due.
2. When a card is due it fires a notification showing the title and body.
3. On **Linux** (dunst), the notification has four action buttons: **Again / Hard / Good / Easy**.
4. On **macOS**, an alert dialog appears with the same four buttons.
5. Your rating feeds back into the FSRS scheduler, which decides when to show the card next — anywhere from a few minutes to months depending on how well you know it.
6. If you dismiss without rating, it defaults to **Good**.

Cards come in two flavours:

- **Tip cards** — title, a prompt (the command or shortcut), and an explanation.
- **Q&A cards** — title (the question) and body (the answer). No prompt.

---

## Prerequisites

### Linux

- A notification daemon that supports actions. [Dunst](https://dunst-project.org/) is recommended and works out of the box on most Wayland/X11 desktops.
- `notify-send` from `libnotify` **0.8 or later** (for interactive action buttons).
  Check your version: `notify-send --version`
- For ntfy push to your phone: a running [ntfy](https://ntfy.sh/) server (self-hosted or ntfy.sh) and a topic URL.

### macOS

No external dependencies. Notifications use `osascript`, which ships with macOS. You may be prompted to allow notifications on first run.

---

## Install

### Download a pre-built binary

Go to the [Releases](../../releases) page and grab the binary for your platform:

| Platform | File |
|---|---|
| Linux x86\_64 | `teacha-daemon-linux-x86_64` |
| macOS Intel | `teacha-daemon-macos-x86_64` |
| macOS Apple Silicon | `teacha-daemon-macos-aarch64` |

```bash
# Linux
chmod +x teacha-daemon-linux-x86_64
mv teacha-daemon-linux-x86_64 ~/.local/bin/teacha-daemon

# macOS — clear the Gatekeeper quarantine flag before running
xattr -d com.apple.quarantine teacha-daemon-macos-aarch64
chmod +x teacha-daemon-macos-aarch64
mv teacha-daemon-macos-aarch64 /usr/local/bin/teacha-daemon
```

### Build from source

Requires Rust (install via [rustup](https://rustup.rs/)).

```bash
git clone https://github.com/robie1373/teacha.git
cd teacha/src-tauri

# Daemon only — no GUI dependencies needed
cargo build --bin teacha-daemon --no-default-features --release
# Binary at: target/release/teacha-daemon
```

With Nix:

```bash
nix develop .#core   # minimal shell: cargo + gcc + openssl
cargo build --bin teacha-daemon --no-default-features --release
```

---

## Quick start

```bash
# Fire any due cards once and exit — good for a first test
teacha-daemon --once

# Run the poll loop (checks every 60 seconds by default)
teacha-daemon

# Explicit channel
teacha-daemon --channels desktop         # Linux (default)
teacha-daemon --channels notification    # macOS (default)

# Desktop + phone push
teacha-daemon --channels desktop,ntfy --ntfy-url https://ntfy.example.com/my-topic

# Custom poll interval
teacha-daemon --poll-seconds 300

# Full help
teacha-daemon --help
```

All options can also be set via environment variables — useful for running as a service:

```bash
export TEACHA_CHANNELS=desktop,ntfy
export TEACHA_NTFY_URL=https://ntfy.example.com/my-topic
export TEACHA_POLL_SECONDS=120
teacha-daemon
```

---

## Notification channels

| Channel | Platform | Interactive? | Notes |
|---|---|---|---|
| `desktop` | Linux | **Yes** | notify-send + dunst; libnotify ≥ 0.8 required |
| `notification` | macOS | **Yes** | osascript alert with rating buttons |
| `ntfy` | Any | No (defaults Good) | HTTP push to `--ntfy-url` / `TEACHA_NTFY_URL` |
| `console` | Any | **Yes** | stdin/stdout; useful for testing |
| `signal` | — | No | stub, not yet implemented |
| `telegram` | — | No | stub, not yet implemented |

Default: `desktop` on Linux, `notification` on macOS.

---

## Card format

Cards are stored in SQLite at:

- **Linux**: `~/.local/share/teacha/cards.db`
- **macOS**: `~/Library/Application Support/teacha/cards.db`

| Field | Required | Description |
|---|---|---|
| `title` | Yes | Notification headline or question |
| `prompt` | No | Command or shortcut (shown before the body) |
| `body` | Yes | Explanation or answer |
| `tags` | No | Comma-separated categories, e.g. `nix,cli` |

**Tip card** (prompt set):
```
title:  comma — run any binary without installing
prompt: , ffmpeg -i input.mp4 output.webm
body:   Uses nix-index-database to locate the package automatically.
tags:   nix,cli
```

**Q&A card** (no prompt):
```
title: What does HTTP 201 mean?
body:  Created. The request succeeded and a new resource was created.
tags:  http
```

---

## Starter deck

On first launch with an empty database, Teacha seeds ~47 cards covering:

- **Nix** — comma, nix shell/run/flake, nh os switch/test
- **Vim** — navigation, text objects, global command, time-travel undo, increment
- **Fish shell** — abbreviations, history search, funced, Alt-.
- **Linux tools** — fd, rg, bat, jq, fzf, sd, hyperfine
- **systemd / process monitoring** — journalctl, systemctl, systemd-analyze, ss, lsof, strace
- **Wayland** — wlr-which-key
- **HTTP** — common status codes

---

## Running as a background service

### systemd user service (Linux)

Create `~/.config/systemd/user/teacha.service`:

```ini
[Unit]
Description=Teacha ambient learning daemon
After=graphical-session.target

[Service]
ExecStart=%h/.local/bin/teacha-daemon
Environment=TEACHA_CHANNELS=desktop
Restart=on-failure

[Install]
WantedBy=default.target
```

Then:
```bash
systemctl --user enable --now teacha.service
journalctl --user -u teacha.service -f
```

### launchd (macOS)

Create `~/Library/LaunchAgents/com.teacha.daemon.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>             <string>com.teacha.daemon</string>
  <key>ProgramArguments</key>  <array><string>/usr/local/bin/teacha-daemon</string></array>
  <key>RunAtLoad</key>         <true/>
  <key>KeepAlive</key>         <true/>
</dict>
</plist>
```

Then:
```bash
launchctl load ~/Library/LaunchAgents/com.teacha.daemon.plist
```

---

## Development

```bash
git clone https://github.com/robie1373/teacha.git
cd teacha

# Daemon + lib tests — no WebKitGTK needed
nix develop .#core
cargo test --lib --no-default-features                # 59 core tests
cargo test --bin teacha-daemon --no-default-features  # 25 daemon tests

# Full Tauri GUI dev environment
nix develop
cargo tauri dev
```

### Project structure

```
teacha/
├── flake.nix              Nix dev shells (default: full Tauri; core: daemon only)
├── src/                   Frontend HTML/CSS/JS
└── src-tauri/
    ├── Cargo.toml         lib + two binaries (teacha GUI, teacha-daemon)
    └── src/
        ├── lib.rs         teacha_core — re-exports db, fsrs, seed
        ├── db.rs          SQLite card store (rusqlite, bundled)
        ├── fsrs.rs        FSRS v5 scheduler (19-parameter weights)
        ├── seed.rs        Starter card deck
        ├── main.rs        Tauri GUI binary (requires gui feature + WebKitGTK)
        └── main_cli.rs    Daemon binary (no GUI deps)
```

---

## Roadmap

- [x] FSRS v5 scheduler
- [x] SQLite card store with full schema (title / prompt / body / tags)
- [x] Daemon: Linux interactive notifications via dunst (Again/Hard/Good/Easy)
- [x] Daemon: macOS Notification Center alert (interactive)
- [x] Daemon: ntfy push notifications
- [x] CLI flags + env var config (`--help` documents everything)
- [x] `--once` flag for one-shot firing (testing, systemd oneshot)
- [x] Starter deck (~47 cards)
- [x] Nix dev shells
- [ ] CLI card management — `add`, `list`, `edit`, `delete`, `import`, `export`
- [ ] Tauri GUI — browse, CRUD, stats dashboard, tag filtering
- [ ] systemd user service module
- [ ] Cross-platform binary releases via GitHub Actions
- [ ] Signal and Telegram notification channels

---

## Credits

- [FSRS algorithm](https://github.com/open-spaced-repetition/fsrs4anki) by the Open Spaced Repetition project
- [Tauri](https://tauri.app) — lightweight Rust + WebView desktop framework
- [Dunst](https://dunst-project.org/) — notification daemon with action support

## License

MIT
