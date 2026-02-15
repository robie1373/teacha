# teacha

A Rust daemon skeleton for spaced repetition reminders over multiple channels with macOS notification support.

## Run

- Build: `cargo build`
- Run: `cargo run`

## Configuration

- `TEACHA_POLL_SECONDS` (default: 60)
- `TEACHA_CHANNELS` (comma list: console, notification, signal, telegram)
- `TEACHA_NOTIFICATION_STYLE` (macOS only: notification, alert)
- `TEACHA_NOTIFICATION_APP` (macOS only: app name or path for Notification Center)

Example:

```
TEACHA_POLL_SECONDS=30 TEACHA_CHANNELS=console,telegram cargo run
```

## Notes

macOS notifications use `osascript` and can be set to `notification` (Notification Center) or `alert` (blocking). To make Notification Center temporary or persistent for teacha specifically, build and use the helper app below and change its alert style in System Settings. Other channels are stubbed and print to stdout.

## macOS helper app (Notification Center style)

Build a tiny notifier app so macOS lets you pick Temporary vs Persistent for teacha:

```
chmod +x scripts/build-macos-notifier.sh
./scripts/build-macos-notifier.sh
```

Then run with:

```
TEACHA_CHANNELS=notification \
TEACHA_NOTIFICATION_STYLE=notification \
TEACHA_NOTIFICATION_APP="./macos/Teacha Notifier.app" \
cargo run
```

Set the alert style in System Settings -> Notifications -> Teacha Notifier.
