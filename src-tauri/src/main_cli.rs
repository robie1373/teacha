use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::io::IsTerminal;
use rand::seq::SliceRandom;
use teacha_core::db::{Database, DbCard};
use teacha_core::fsrs::{Rating, State};
use teacha_core::seed::seed_if_empty;

const DEFAULT_POLL_SECS: u64 = 60;

// ── CLI args ──────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "teacha-daemon",
    version,
    about = "FSR-scheduled card notifications for ambient CLI/vim/tool learning.",
    long_about = "Without a subcommand: runs the notification poll loop.\n\
\nAccepted channel names (comma-separated):\n\
\n  desktop      system notifications (notify-send on Linux, osascript on macOS)\
\n  ntfy         HTTP push to --ntfy-url / TEACHA_NTFY_URL\
\n  console      interactive stdin/stdout\
\n\
\nFlags that show [env: ...] can also be set as environment variables.\n\
\nExample — set once in your shell config (~/.bashrc, ~/.zshrc, config.fish):\n\
\n  export TEACHA_CHANNELS=desktop,ntfy\
\n  export TEACHA_NTFY_URL=https://ntfy.example.com/my-topic\
\n  export TEACHA_POLL_SECONDS=120\
\n\
\nAny flag passed on the command line overrides the environment variable."
)]
struct Args {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// Notification channels to use (comma-separated).
    /// Default: desktop on Linux, notification on macOS.
    #[arg(
        long,
        env = "TEACHA_CHANNELS",
        value_delimiter = ',',
        value_name = "CHANNEL",
        help_heading = "Daemon options"
    )]
    channels: Option<Vec<String>>,

    /// Seconds between polling the database for due cards.
    #[arg(
        long,
        env = "TEACHA_POLL_SECONDS",
        default_value_t = DEFAULT_POLL_SECS,
        value_name = "SECS",
        help_heading = "Daemon options"
    )]
    poll_seconds: u64,

    /// ntfy endpoint URL including topic, e.g. https://ntfy.example.com/teacha
    #[arg(
        long,
        env = "TEACHA_NTFY_URL",
        value_name = "URL",
        help_heading = "Daemon options"
    )]
    ntfy_url: Option<String>,

    /// Fire all due cards once and exit (no poll loop).
    /// Useful for testing and systemd oneshot units.
    #[arg(long, help_heading = "Daemon options")]
    once: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Add a new card to the database.
    Add {
        /// Card headline or question.
        #[arg(long, short)]
        title: String,
        /// Command or shortcut to remember (tip cards only; omit for Q&A cards).
        #[arg(long, short)]
        prompt: Option<String>,
        /// Explanation or answer.
        #[arg(long, short)]
        body: String,
        /// Comma-separated tags, e.g. nix,cli
        #[arg(long, short = 'T', default_value = "")]
        tags: String,
    },

    /// List cards in the database.
    List {
        /// Filter by tag (substring match).
        #[arg(long, short)]
        tag: Option<String>,
        /// Show only cards that are currently due.
        #[arg(long, short)]
        due: bool,
    },

    /// Edit an existing card by ID.
    Edit {
        /// Card ID (from `teacha-daemon list`).
        id: i64,
        /// New title.
        #[arg(long, short)]
        title: Option<String>,
        /// New prompt (set to update; use --clear-prompt to remove).
        #[arg(long, short)]
        prompt: Option<String>,
        /// Remove the prompt field (converts tip card to Q&A card).
        #[arg(long)]
        clear_prompt: bool,
        /// New body.
        #[arg(long, short)]
        body: Option<String>,
        /// New tags.
        #[arg(long, short = 'T')]
        tags: Option<String>,
    },

    /// Delete a card by ID.
    Delete {
        /// Card ID (from `teacha-daemon list`).
        id: i64,
        /// Skip confirmation prompt.
        #[arg(long, short)]
        yes: bool,
    },

    /// Import cards from a JSON file.
    ///
    /// Expected format: array of objects with title, body,
    /// and optional prompt and tags fields.
    Import {
        /// Path to JSON file (use - for stdin).
        file: PathBuf,
    },

    /// Export all cards to JSON.
    Export {
        /// Write to file instead of stdout.
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
}

// ── Card management commands ──────────────────────────────────────────────────

fn cmd_add(db: &Database, title: &str, prompt: Option<&str>, body: &str, tags: &str) {
    match db.add_card(title, prompt, body, tags) {
        Ok(id) => println!("Added card #{id}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn cmd_list(db: &Database, tag: Option<&str>, due_only: bool) {
    let cards = if let Some(t) = tag {
        db.get_cards_by_tag(t).unwrap_or_default()
    } else {
        db.get_all_cards().unwrap_or_default()
    };

    let now = now_unix();
    let cards: Vec<_> = if due_only {
        cards.into_iter().filter(|c| c.due_at <= now).collect()
    } else {
        cards
    };

    if cards.is_empty() {
        println!("No cards found.");
        return;
    }

    println!("{:>4}  {:<48}  {:<14}  {:<10}  {}", "ID", "TITLE", "TAGS", "STATE", "DUE");
    println!("{}", "-".repeat(90));

    for card in &cards {
        let title = truncate(&card.title, 48);
        let tags = truncate(&card.tags, 14);
        let state = format!("{:?}", card.fsrs_state.state);
        let due = format_due(card.due_at, now);
        println!("{:>4}  {:<48}  {:<14}  {:<10}  {}", card.id, title, tags, state, due);
    }

    println!("\n{} card(s)", cards.len());
}

fn cmd_edit(
    db: &Database,
    id: i64,
    new_title: Option<&str>,
    new_prompt: Option<&str>,
    clear_prompt: bool,
    new_body: Option<&str>,
    new_tags: Option<&str>,
) {
    let card = match db.get_card(id) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: no card with ID {id}");
            return;
        }
    };

    let title = new_title.unwrap_or(&card.title);
    let prompt = if clear_prompt {
        None
    } else if let Some(p) = new_prompt {
        Some(p)
    } else {
        card.prompt.as_deref()
    };
    let body = new_body.unwrap_or(&card.body);
    let tags_str;
    let tags = if let Some(t) = new_tags {
        t
    } else {
        tags_str = card.tags.clone();
        &tags_str
    };

    match db.update_card(id, title, prompt, body, tags) {
        Ok(()) => println!("Updated card #{id}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn cmd_delete(db: &Database, id: i64, yes: bool) {
    if !yes {
        // Fetch card so we can show the title in the confirmation
        match db.get_card(id) {
            Ok(card) => println!("Delete card #{id}: \"{}\"? Pass --yes to confirm.", card.title),
            Err(_) => eprintln!("Error: no card with ID {id}"),
        }
        return;
    }
    match db.delete_card(id) {
        Ok(()) => println!("Deleted card #{id}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}

// ── Import / Export ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct CardRecord {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<String>,
    body: String,
    #[serde(default)]
    tags: String,
}

impl From<&DbCard> for CardRecord {
    fn from(c: &DbCard) -> Self {
        CardRecord {
            title: c.title.clone(),
            prompt: c.prompt.clone(),
            body: c.body.clone(),
            tags: c.tags.clone(),
        }
    }
}

fn cmd_import(db: &Database, file: &PathBuf) {
    let json = if file.to_string_lossy() == "-" {
        let mut s = String::new();
        use std::io::Read;
        std::io::stdin().read_to_string(&mut s).expect("read stdin");
        s
    } else {
        std::fs::read_to_string(file).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {e}", file.display());
            std::process::exit(1);
        })
    };

    let records: Vec<CardRecord> = serde_json::from_str(&json).unwrap_or_else(|e| {
        eprintln!("Error parsing JSON: {e}");
        std::process::exit(1);
    });

    let mut added = 0usize;
    for r in &records {
        match db.add_card(&r.title, r.prompt.as_deref(), &r.body, &r.tags) {
            Ok(_) => added += 1,
            Err(e) => eprintln!("Skipping \"{}\": {e}", r.title),
        }
    }
    println!("Imported {added}/{} card(s)", records.len());
}

fn cmd_export(db: &Database, output: Option<&PathBuf>) {
    let cards = db.get_all_cards().unwrap_or_default();
    let records: Vec<CardRecord> = cards.iter().map(CardRecord::from).collect();
    let json = serde_json::to_string_pretty(&records).expect("serialize");

    match output {
        Some(path) => {
            std::fs::write(path, &json).unwrap_or_else(|e| {
                eprintln!("Error writing {}: {e}", path.display());
                std::process::exit(1);
            });
            println!("Exported {} card(s) to {}", records.len(), path.display());
        }
        None => println!("{json}"),
    }
}

// ── Formatting helpers ────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        format!("{}…", chars[..max - 1].iter().collect::<String>())
    }
}

fn format_due(due_at: i64, now: i64) -> String {
    let diff = due_at - now;
    if diff <= 0 {
        return "now".to_string();
    }
    let secs = diff as u64;
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        3600..=86399 => format!("{}h", secs / 3600),
        86400..=604799 => format!("{}d", secs / 86400),
        _ => format!("{}w", secs / 604800),
    }
}

// ── ANSI color helpers ────────────────────────────────────────────────────────

fn tty() -> bool {
    std::io::stdout().is_terminal()
}

fn bold(s: &str) -> String {
    if tty() { format!("\x1b[1m{s}\x1b[0m") } else { s.to_string() }
}

fn dim(s: &str) -> String {
    if tty() { format!("\x1b[2m{s}\x1b[0m") } else { s.to_string() }
}

fn colored(s: &str, code: u8) -> String {
    if tty() { format!("\x1b[{}m{s}\x1b[0m", code) } else { s.to_string() }
}

fn blue(s: &str) -> String   { colored(s, 34) }
fn yellow(s: &str) -> String { colored(s, 33) }
fn green(s: &str) -> String  { colored(s, 32) }
fn red(s: &str) -> String    { colored(s, 31) }

// ── Notifier trait ────────────────────────────────────────────────────────────

trait Notifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating>;
    fn session_header(&self, _db: &Database, _due_count: usize) {}
}

// ── Console ───────────────────────────────────────────────────────────────────

struct ConsoleNotifier;

impl Notifier for ConsoleNotifier {
    fn session_header(&self, db: &Database, due_count: usize) {
        let cards = db.get_all_cards().unwrap_or_default();
        let new      = cards.iter().filter(|c| c.fsrs_state.state == State::New).count();
        let learning = cards.iter().filter(|c| c.fsrs_state.state == State::Learning).count();
        let review   = cards.iter().filter(|c| c.fsrs_state.state == State::Review).count();
        let relearn  = cards.iter().filter(|c| c.fsrs_state.state == State::Relearning).count();

        println!();
        println!(
            "{}  {}  {}  {}  {}",
            bold(&format!("{} due", due_count)),
            blue(&format!("{} new", new)),
            yellow(&format!("{} learning", learning)),
            green(&format!("{} review", review)),
            red(&format!("{} relearning", relearn)),
        );
        println!("{}", dim("─────────────────────────────────────────"));
    }

    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        println!();
        println!("{}", dim("─────────────────────────────────────────"));
        println!("{}", bold(title));
        println!("  {body}");
        print!("  {}", dim("(1) Again  (2) Hard  (3) Good  (4) Easy  › "));
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            Rating::from_str(input.trim())
        } else {
            None
        }
    }
}

// ── Desktop (Linux: notify-send / dunst) ──────────────────────────────────────

struct DesktopNotifier;

impl Notifier for DesktopNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        let output = Command::new("notify-send")
            .args([
                "--app-name=Teacha",
                "--wait",
                "--action=1,Again",
                "--action=2,Hard",
                "--action=3,Good",
                "--action=4,Easy",
                title,
                body,
            ])
            .output();

        match output {
            Ok(out) => Rating::from_str(String::from_utf8_lossy(&out.stdout).trim()),
            Err(e) => {
                eprintln!("[desktop] notify-send failed: {e}");
                None
            }
        }
    }
}

// ── ntfy (HTTP push) ──────────────────────────────────────────────────────────

struct NtfyNotifier {
    url: String,
}

impl Notifier for NtfyNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        if let Err(e) = ureq::post(&self.url).set("Title", title).send_string(body) {
            eprintln!("[ntfy] failed to send: {e}");
        }
        None
    }
}

// ── macOS Notification Center ─────────────────────────────────────────────────

#[cfg(target_os = "macos")]
struct NotificationCenterNotifier;

#[cfg(target_os = "macos")]
impl Notifier for NotificationCenterNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        self.send_alert_with_rating(title, body)
    }
}

#[cfg(target_os = "macos")]
impl NotificationCenterNotifier {
    fn send_alert_with_rating(&self, title: &str, body: &str) -> Option<Rating> {
        let script = format!(
            "display alert \"{}\" message \"{}\" buttons {{\"Again\", \"Hard\", \"Good\", \"Easy\"}} default button \"Good\"",
            escape_osascript(title),
            escape_osascript(body)
        );
        match Command::new("osascript").arg("-e").arg(&script).output() {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout
                    .split(':')
                    .nth(1)
                    .and_then(|b| Rating::from_str(b.trim()))
                    .or(Some(Rating::Good))
            }
            Ok(_) => {
                eprintln!("[notification] alert dismissed");
                None
            }
            Err(e) => {
                eprintln!("[notification] osascript failed: {e}");
                None
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn escape_osascript(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
}

// ── Channel enum ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Channel {
    Console,
    Desktop,
    Ntfy,
}

impl Channel {
    fn parse_list(raw: &[String]) -> Vec<Channel> {
        raw.iter()
            .map(|v| v.trim().to_lowercase())
            .filter_map(|v| match v.as_str() {
                "console" => Some(Channel::Console),
                "desktop" | "notification" | "notifications" => Some(Channel::Desktop),
                "ntfy" => Some(Channel::Ntfy),
                other => {
                    eprintln!("unknown channel {other:?}, ignoring");
                    None
                }
            })
            .collect()
    }

    fn defaults() -> Vec<Channel> {
        vec![Channel::Desktop]
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

fn build_notifiers(channels: &[Channel], ntfy_url: Option<&str>) -> Vec<Box<dyn Notifier>> {
    let mut notifiers: Vec<Box<dyn Notifier>> = Vec::new();
    for channel in channels {
        match channel {
            Channel::Console => notifiers.push(Box::new(ConsoleNotifier)),
            Channel::Desktop => {
                #[cfg(target_os = "macos")]
                notifiers.push(Box::new(NotificationCenterNotifier));
                #[cfg(not(target_os = "macos"))]
                notifiers.push(Box::new(DesktopNotifier));
            }
            Channel::Ntfy => match ntfy_url {
                Some(url) => notifiers.push(Box::new(NtfyNotifier { url: url.to_string() })),
                None => eprintln!("[ntfy] --ntfy-url / TEACHA_NTFY_URL not set, skipping"),
            },
        }
    }
    if notifiers.is_empty() {
        notifiers.push(Box::new(ConsoleNotifier));
    }
    notifiers
}

// ── Notification content ──────────────────────────────────────────────────────

fn card_notification_body(card: &DbCard) -> String {
    match &card.prompt {
        Some(prompt) => format!("{}\n\n{}", prompt, card.body),
        None => card.body.clone(),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();
    let db = Database::new().expect("Failed to open database");

    match args.cmd {
        Some(Cmd::Add { title, prompt, body, tags }) => {
            cmd_add(&db, &title, prompt.as_deref(), &body, &tags);
            return;
        }
        Some(Cmd::List { tag, due }) => {
            cmd_list(&db, tag.as_deref(), due);
            return;
        }
        Some(Cmd::Edit { id, title, prompt, clear_prompt, body, tags }) => {
            cmd_edit(&db, id, title.as_deref(), prompt.as_deref(), clear_prompt, body.as_deref(), tags.as_deref());
            return;
        }
        Some(Cmd::Delete { id, yes }) => {
            cmd_delete(&db, id, yes);
            return;
        }
        Some(Cmd::Import { file }) => {
            cmd_import(&db, &file);
            return;
        }
        Some(Cmd::Export { output }) => {
            cmd_export(&db, output.as_ref());
            return;
        }
        None => {}
    }

    // Daemon mode
    seed_if_empty(&db);

    let channels = args
        .channels
        .map(|raw| Channel::parse_list(&raw))
        .filter(|c| !c.is_empty())
        .unwrap_or_else(Channel::defaults);

    let notifiers = build_notifiers(&channels, args.ntfy_url.as_deref());

    if args.once {
        println!("teacha-daemon: firing due cards once");
        fire_due_cards(&db, &notifiers);
        return;
    }

    println!("teacha-daemon: poll={}s channels={:?}", args.poll_seconds, channels);

    loop {
        fire_due_cards(&db, &notifiers);
        thread::sleep(Duration::from_secs(args.poll_seconds));
    }
}

fn fire_due_cards(db: &Database, notifiers: &[Box<dyn Notifier>]) {
    let now = now_unix();
    let mut due_cards = db.get_due_cards(now).unwrap_or_default();
    due_cards.shuffle(&mut rand::thread_rng());
    for n in notifiers {
        n.session_header(db, due_cards.len());
    }
    for card in due_cards {
        let title = card.title.clone();
        let body = card_notification_body(&card);
        let rating = notifiers
            .iter()
            .find_map(|n| n.send(&title, &body))
            .unwrap_or(Rating::Good);
        if let Err(e) = db.review_card(card.id, rating) {
            eprintln!("review_card failed for id={}: {e}", card.id);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use teacha_core::db::Database;
    use teacha_core::fsrs::CardState;

    fn test_db() -> Database {
        Database::open_in_memory().expect("test db")
    }

    // ── Channel parsing ──────────────────────────────────────────────

    fn ch(s: &str) -> Vec<Channel> {
        Channel::parse_list(&[s.to_string()])
    }

    fn chs(s: &str) -> Vec<Channel> {
        Channel::parse_list(&s.split(',').map(|v| v.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn parse_console_channel() {
        assert!(matches!(ch("console")[0], Channel::Console));
    }

    #[test]
    fn parse_desktop_channel() {
        assert!(matches!(ch("desktop")[0], Channel::Desktop));
    }

    #[test]
    fn parse_ntfy_channel() {
        assert!(matches!(ch("ntfy")[0], Channel::Ntfy));
    }

    #[test]
    fn parse_multiple_channels() {
        assert_eq!(chs("console,desktop,ntfy").len(), 3);
    }

    #[test]
    fn parse_channels_case_insensitive() {
        assert_eq!(chs("CONSOLE,Desktop,NTFY").len(), 3);
    }

    #[test]
    fn parse_notifications_alias() {
        assert!(matches!(ch("notifications")[0], Channel::Desktop));
    }

    #[test]
    fn parse_notification_alias() {
        assert!(matches!(ch("notification")[0], Channel::Desktop));
    }

    #[test]
    fn parse_channels_unknown_ignored() {
        assert_eq!(chs("console,fax").len(), 1);
    }

    #[test]
    fn parse_channels_empty_vec() {
        assert!(Channel::parse_list(&[]).is_empty());
    }

    // ── Rating parsing ───────────────────────────────────────────────

    #[test]
    fn rating_from_str_words() {
        assert_eq!(Rating::from_str("again"), Some(Rating::Again));
        assert_eq!(Rating::from_str("good"), Some(Rating::Good));
        assert_eq!(Rating::from_str("easy"), Some(Rating::Easy));
    }

    #[test]
    fn rating_from_str_numbers() {
        assert_eq!(Rating::from_str("1"), Some(Rating::Again));
        assert_eq!(Rating::from_str("4"), Some(Rating::Easy));
    }

    #[test]
    fn rating_from_str_invalid() {
        assert_eq!(Rating::from_str(""), None);
        assert_eq!(Rating::from_str("5"), None);
    }

    // ── Notifiers ────────────────────────────────────────────────────

    #[test]
    fn desktop_notifier_does_not_panic() {
        let _ = DesktopNotifier.send("t", "b");
    }

    // ── build_notifiers ──────────────────────────────────────────────

    #[test]
    fn build_notifiers_empty_falls_back_to_console() {
        assert_eq!(build_notifiers(&[], None).len(), 1);
    }

    #[test]
    fn build_notifiers_ntfy_without_url_falls_back_to_console() {
        assert_eq!(build_notifiers(&[Channel::Ntfy], None).len(), 1);
    }

    #[test]
    fn build_notifiers_ntfy_with_url() {
        assert_eq!(build_notifiers(&[Channel::Ntfy], Some("https://ntfy.example.com/t")).len(), 1);
    }

    // ── card_notification_body ───────────────────────────────────────

    fn make_card(prompt: Option<&str>, body: &str) -> DbCard {
        DbCard {
            id: 1,
            title: "test".to_string(),
            prompt: prompt.map(|s| s.to_string()),
            body: body.to_string(),
            tags: "".to_string(),
            fsrs_state: CardState::new(),
            due_at: 0,
            created_at: 0,
        }
    }

    #[test]
    fn tip_card_body_includes_prompt_and_body() {
        let body = card_notification_body(&make_card(Some(", ffmpeg ..."), "Uses nix-index."));
        assert!(body.contains(", ffmpeg"));
        assert!(body.contains("nix-index"));
    }

    #[test]
    fn qa_card_body_is_body_only() {
        assert_eq!(card_notification_body(&make_card(None, "Resource created.")), "Resource created.");
    }

    // ── format_due ───────────────────────────────────────────────────

    #[test]
    fn format_due_past_is_now() {
        assert_eq!(format_due(100, 200), "now");
    }

    #[test]
    fn format_due_seconds() {
        assert_eq!(format_due(130, 100), "30s");
    }

    #[test]
    fn format_due_minutes() {
        assert_eq!(format_due(100 + 120, 100), "2m");
    }

    #[test]
    fn format_due_hours() {
        assert_eq!(format_due(100 + 7200, 100), "2h");
    }

    #[test]
    fn format_due_days() {
        assert_eq!(format_due(100 + 86400 * 3, 100), "3d");
    }

    #[test]
    fn format_due_weeks() {
        assert_eq!(format_due(100 + 604800 * 2, 100), "2w");
    }

    // ── truncate ─────────────────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string_gets_ellipsis() {
        let result = truncate("hello world", 8);
        assert!(result.ends_with('…'));
        assert!(result.chars().count() <= 8);
    }

    // ── cmd_add / cmd_list / cmd_edit / cmd_delete ───────────────────

    #[test]
    fn cmd_add_inserts_card() {
        let db = test_db();
        cmd_add(&db, "vim G", Some("G"), "Jump to last line", "vim");
        assert_eq!(db.get_all_cards().unwrap().len(), 1);
    }

    #[test]
    fn cmd_add_qa_card() {
        let db = test_db();
        cmd_add(&db, "HTTP 201?", None, "Resource created.", "http");
        let card = &db.get_all_cards().unwrap()[0];
        assert_eq!(card.prompt, None);
    }

    #[test]
    fn cmd_list_runs_without_panic() {
        let db = test_db();
        cmd_add(&db, "title", None, "body", "tag");
        cmd_list(&db, None, false);
        cmd_list(&db, Some("tag"), false);
        cmd_list(&db, None, true);
    }

    #[test]
    fn cmd_edit_updates_title() {
        let db = test_db();
        cmd_add(&db, "old title", None, "body", "");
        let id = db.get_all_cards().unwrap()[0].id;
        cmd_edit(&db, id, Some("new title"), None, false, None, None);
        assert_eq!(db.get_card(id).unwrap().title, "new title");
    }

    #[test]
    fn cmd_edit_clear_prompt() {
        let db = test_db();
        cmd_add(&db, "title", Some("cmd"), "body", "");
        let id = db.get_all_cards().unwrap()[0].id;
        cmd_edit(&db, id, None, None, true, None, None);
        assert_eq!(db.get_card(id).unwrap().prompt, None);
    }

    #[test]
    fn cmd_edit_nonexistent_id_does_not_panic() {
        let db = test_db();
        cmd_edit(&db, 9999, Some("title"), None, false, Some("body"), None);
    }

    #[test]
    fn cmd_delete_without_yes_does_not_delete() {
        let db = test_db();
        cmd_add(&db, "title", None, "body", "");
        let id = db.get_all_cards().unwrap()[0].id;
        cmd_delete(&db, id, false);
        assert_eq!(db.get_all_cards().unwrap().len(), 1);
    }

    #[test]
    fn cmd_delete_with_yes_deletes() {
        let db = test_db();
        cmd_add(&db, "title", None, "body", "");
        let id = db.get_all_cards().unwrap()[0].id;
        cmd_delete(&db, id, true);
        assert_eq!(db.get_all_cards().unwrap().len(), 0);
    }

    // ── import / export roundtrip ────────────────────────────────────

    #[test]
    fn export_then_import_roundtrip() {
        let db1 = test_db();
        cmd_add(&db1, "comma tip", Some(", ffmpeg ..."), "Uses nix-index.", "nix,cli");
        cmd_add(&db1, "HTTP 201?", None, "Resource created.", "http");

        let cards = db1.get_all_cards().unwrap();
        let records: Vec<CardRecord> = cards.iter().map(CardRecord::from).collect();
        let json = serde_json::to_string(&records).unwrap();

        let db2 = test_db();
        let records2: Vec<CardRecord> = serde_json::from_str(&json).unwrap();
        for r in &records2 {
            db2.add_card(&r.title, r.prompt.as_deref(), &r.body, &r.tags).unwrap();
        }

        let imported = db2.get_all_cards().unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].title, cards[0].title);
        assert_eq!(imported[1].prompt, cards[1].prompt);
    }

    #[test]
    fn card_record_serializes_without_prompt() {
        let record = CardRecord {
            title: "Q?".to_string(),
            prompt: None,
            body: "A.".to_string(),
            tags: "".to_string(),
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("prompt"));
    }

    // ── macOS escape (gated) ─────────────────────────────────────────

    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::*;

        #[test]
        fn escape_quotes() {
            assert_eq!(escape_osascript(r#"say "hi""#), r#"say \"hi\""#);
        }

        #[test]
        fn escape_newlines() {
            assert_eq!(escape_osascript("a\nb"), r"a\nb");
        }
    }
}
