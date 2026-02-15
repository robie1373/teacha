# teacha

A Rust daemon skeleton for spaced repetition reminders over multiple channels.

## Run

- Build: `cargo build`
- Run: `cargo run`

## Configuration

- `TEACHA_POLL_SECONDS` (default: 60)
- `TEACHA_CHANNELS` (comma list: console, notification, signal, telegram)

Example:

```
TEACHA_POLL_SECONDS=30 TEACHA_CHANNELS=console,telegram cargo run
```

## Notes

This version uses stub notifiers that print to stdout. Replace them with real integrations as needed.
