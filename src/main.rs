mod fsrs;

use fsrs::{CardState, Rating};
use std::env;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(target_os = "macos")]
use std::process::Command;

const DEFAULT_POLL_SECS: u64 = 60;
const DEFAULT_HELPER_APP: &str = "./macos/Teacha Notifier.app";

#[derive(Clone, Debug)]
struct MemoryItem {
    prompt: String,
    answer: String,
    card: CardState,
    due_at: u64,
}

struct MemoryStore {
    items: Vec<MemoryItem>,
}

impl MemoryStore {
    fn sample(now: u64) -> Self {
        Self {
            items: vec![
                MemoryItem {
                    prompt: "Rust ownership rule?".to_string(),
                    answer: "Each value has a single owner at a time.".to_string(),
                    card: CardState::new(),
                    due_at: now,
                },
                MemoryItem {
                    prompt: "HTTP 201 means?".to_string(),
                    answer: "Resource created.".to_string(),
                    card: CardState::new(),
                    due_at: now + 5,
                },
            ],
        }
    }

    fn due_items(&self, now: u64) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| if item.due_at <= now { Some(index) } else { None })
            .collect()
    }

    fn review(&mut self, index: usize, rating: Rating, now: u64) {
        if let Some(item) = self.items.get_mut(index) {
            let now_days = now as f64 / 86400.0;
            let (next_card, interval_secs) = item.card.review(rating, now_days);
            item.card = next_card;
            item.due_at = now + interval_secs;
            println!(
                "  -> rated {:?}, next in {}s (S={:.2} D={:.2})",
                rating, interval_secs, item.card.stability, item.card.difficulty
            );
        }
    }
}

trait Notifier {
    /// Send a review prompt and collect a rating from the user.
    /// Returns None if the channel cannot collect interactive feedback.
    fn send(&self, title: &str, body: &str) -> Option<Rating>;
}

struct ConsoleNotifier;

impl Notifier for ConsoleNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        println!("[console] {title}");
        println!("  {body}");
        println!("  Rate: (1) Again  (2) Hard  (3) Good  (4) Easy");
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            Rating::from_str(input.trim())
        } else {
            None
        }
    }
}

struct SignalNotifier;

impl Notifier for SignalNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        println!("[signal] {title} - {body}");
        None
    }
}

struct TelegramNotifier;

impl Notifier for TelegramNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        println!("[telegram] {title} - {body}");
        None
    }
}

struct NotificationCenterNotifier {
    style: NotificationStyle,
    app: Option<String>,
}
#[derive(Clone, Copy, Debug)]
enum NotificationStyle {
    Alert,
    Notification,
}

impl Notifier for NotificationCenterNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        #[cfg(target_os = "macos")]
        {
            match self.style {
                NotificationStyle::Alert => {
                    return self.send_alert_with_rating(title, body);
                }
                NotificationStyle::Notification => {
                    if let Some(app) = &self.app {
                        if let Err(error) = send_notification_via_app(app, title, body) {
                            eprintln!("[notification] helper app failed: {error}");
                        }
                    } else {
                        let escaped_title = escape_osascript(title);
                        let escaped_body = escape_osascript(body);
                        let script = format!(
                            "display notification \"{}\" with title \"{}\"",
                            escaped_body, escaped_title
                        );
                        if let Err(error) =
                            Command::new("osascript").arg("-e").arg(script).status()
                        {
                            eprintln!("[notification] osascript failed: {error}");
                        }
                    }
                    return None; // non-interactive
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            println!("[notification] {title} - {body}");
            return None;
        }
    }
}

#[cfg(target_os = "macos")]
impl NotificationCenterNotifier {
    fn send_alert_with_rating(&self, title: &str, body: &str) -> Option<Rating> {
        let escaped_title = escape_osascript(title);
        let escaped_body = escape_osascript(body);
        let script = format!(
            "display alert \"{}\" message \"{}\" buttons {{\"Again\", \"Hard\", \"Good\", \"Easy\"}} default button \"Good\"",
            escaped_title, escaped_body
        );
        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // osascript returns e.g. "button returned:Good"
                if let Some(button) = stdout.split(':').nth(1) {
                    Rating::from_str(button.trim())
                } else {
                    Some(Rating::Good)
                }
            }
            Ok(_) => {
                eprintln!("[notification] alert dismissed or cancelled");
                None
            }
            Err(error) => {
                eprintln!("[notification] osascript failed: {error}");
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

#[derive(Clone, Copy, Debug)]
enum Channel {
    Console,
    Signal,
    Telegram,
    Notification,
}

impl Channel {
    fn parse_list(raw: &str) -> Vec<Channel> {
        raw.split(',')
            .map(|value| value.trim().to_lowercase())
            .filter_map(|value| match value.as_str() {
                "console" => Some(Channel::Console),
                "signal" => Some(Channel::Signal),
                "telegram" => Some(Channel::Telegram),
                "notification" | "notifications" => Some(Channel::Notification),
                _ => None,
            })
            .collect()
    }
}

fn build_notifiers(channels: &[Channel]) -> Vec<Box<dyn Notifier>> {
    let notification_style = read_notification_style();
    let notification_app = read_notification_app();
    let mut notifiers: Vec<Box<dyn Notifier>> = Vec::new();
    let mut add_console_fallback = false;
    for channel in channels {
        match channel {
            Channel::Console => notifiers.push(Box::new(ConsoleNotifier)),
            Channel::Signal => notifiers.push(Box::new(SignalNotifier)),
            Channel::Telegram => notifiers.push(Box::new(TelegramNotifier)),
            Channel::Notification => notifiers.push(Box::new(NotificationCenterNotifier {
                style: notification_style,
                app: notification_app.clone(),
            })),
        }
        if matches!(channel, Channel::Notification)
            && matches!(notification_style, NotificationStyle::Notification)
            && notification_app.is_none()
        {
            add_console_fallback = true;
        }
    }
    #[cfg(target_os = "macos")]
    if add_console_fallback {
        notifiers.push(Box::new(ConsoleNotifier));
    }
    if notifiers.is_empty() {
        notifiers.push(Box::new(ConsoleNotifier));
    }
    notifiers
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_poll_seconds() -> u64 {
    env::var("TEACHA_POLL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_POLL_SECS)
}

fn read_channels() -> Vec<Channel> {
    env::var("TEACHA_CHANNELS")
        .ok()
        .map(|value| Channel::parse_list(&value))
        .filter(|channels| !channels.is_empty())
    .unwrap_or_else(|| vec![Channel::Notification])
}

fn read_notification_style() -> NotificationStyle {
    match env::var("TEACHA_NOTIFICATION_STYLE")
        .ok()
        .map(|value| value.trim().to_lowercase())
        .as_deref()
    {
        Some("alert") => NotificationStyle::Alert,
        Some("notification") => NotificationStyle::Notification,
        _ => NotificationStyle::Notification,
    }
}

fn read_notification_app() -> Option<String> {
    env::var("TEACHA_NOTIFICATION_APP")
        .ok()
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .or_else(|| {
            if Path::new(DEFAULT_HELPER_APP).exists() {
                Some(DEFAULT_HELPER_APP.to_string())
            } else {
                None
            }
        })
}

#[cfg(target_os = "macos")]
fn send_notification_via_app(app: &str, title: &str, body: &str) -> Result<(), std::io::Error> {
    let status = Command::new("open")
        .arg("-a")
        .arg(app)
        .arg("--args")
        .arg(title)
        .arg(body)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "helper app returned non-zero status",
        ))
    }
}

fn main() {
    let poll_seconds = read_poll_seconds();
    let channels = read_channels();
    let notification_app = read_notification_app();
    let notifiers = build_notifiers(&channels);
    let mut store = MemoryStore::sample(now_unix());

    println!("teacha running. poll={}s channels={:?}", poll_seconds, channels);
    if channels.iter().any(|channel| matches!(channel, Channel::Notification)) {
        match notification_app {
            Some(path) => println!("notification app: {path}"),
            None => println!("notification app: none"),
        }
    }

    loop {
        let now = now_unix();
        let due_indices = store.due_items(now);
        for index in due_indices {
            let (title, body) = {
                let item = &store.items[index];
                (
                    format!("Review: {}", item.prompt),
                    format!("Answer: {}", item.answer),
                )
            };

            // Collect a rating from the first notifier that returns one.
            let mut rating: Option<Rating> = None;
            for notifier in &notifiers {
                if let Some(r) = notifier.send(&title, &body) {
                    rating = Some(r);
                    break;
                }
            }

            // Default to Good if no interactive feedback.
            let rating = rating.unwrap_or(Rating::Good);
            store.review(index, rating, now);
        }

        thread::sleep(Duration::from_secs(poll_seconds));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── MemoryStore ──────────────────────────────────────────────

    #[test]
    fn sample_store_has_two_items() {
        let store = MemoryStore::sample(1000);
        assert_eq!(store.items.len(), 2);
    }

    #[test]
    fn sample_store_items_are_new_cards() {
        let store = MemoryStore::sample(1000);
        for item in &store.items {
            assert_eq!(item.card.state, fsrs::State::New);
            assert_eq!(item.card.reps, 0);
        }
    }

    #[test]
    fn due_items_returns_only_due() {
        let store = MemoryStore::sample(1000);
        // First item due at 1000, second at 1005
        let due = store.due_items(1000);
        assert_eq!(due, vec![0]);

        let due = store.due_items(1005);
        assert_eq!(due, vec![0, 1]);
    }

    #[test]
    fn due_items_returns_empty_before_due() {
        let store = MemoryStore::sample(1000);
        let due = store.due_items(999);
        assert!(due.is_empty());
    }

    #[test]
    fn review_updates_card_state() {
        let mut store = MemoryStore::sample(1000);
        store.review(0, Rating::Good, 1000);
        assert_eq!(store.items[0].card.reps, 1);
        assert_eq!(store.items[0].card.state, fsrs::State::Review);
        assert!(store.items[0].due_at > 1000);
    }

    #[test]
    fn review_again_sets_learning() {
        let mut store = MemoryStore::sample(1000);
        store.review(0, Rating::Again, 1000);
        assert_eq!(store.items[0].card.state, fsrs::State::Learning);
        assert_eq!(store.items[0].card.lapses, 1);
    }

    #[test]
    fn review_easy_gives_longer_interval_than_hard() {
        let mut store_easy = MemoryStore::sample(1000);
        let mut store_hard = MemoryStore::sample(1000);
        store_easy.review(0, Rating::Easy, 1000);
        store_hard.review(0, Rating::Hard, 1000);
        assert!(
            store_easy.items[0].due_at >= store_hard.items[0].due_at,
            "easy due_at={} should >= hard due_at={}",
            store_easy.items[0].due_at,
            store_hard.items[0].due_at
        );
    }

    #[test]
    fn review_out_of_bounds_does_nothing() {
        let mut store = MemoryStore::sample(1000);
        store.review(99, Rating::Good, 1000); // should not panic
        assert_eq!(store.items[0].card.reps, 0);
    }

    #[test]
    fn multiple_reviews_grow_interval() {
        let mut store = MemoryStore::sample(0);
        store.review(0, Rating::Good, 0);
        let due1 = store.items[0].due_at;
        store.review(0, Rating::Good, due1);
        let due2 = store.items[0].due_at;
        let interval1 = due1;
        let interval2 = due2 - due1;
        assert!(
            interval2 > interval1,
            "interval2={} should > interval1={}",
            interval2, interval1
        );
    }

    // ── Channel parsing ─────────────────────────────────────────

    #[test]
    fn parse_single_channel() {
        let channels = Channel::parse_list("console");
        assert_eq!(channels.len(), 1);
        assert!(matches!(channels[0], Channel::Console));
    }

    #[test]
    fn parse_multiple_channels() {
        let channels = Channel::parse_list("console,signal,telegram,notification");
        assert_eq!(channels.len(), 4);
        assert!(matches!(channels[0], Channel::Console));
        assert!(matches!(channels[1], Channel::Signal));
        assert!(matches!(channels[2], Channel::Telegram));
        assert!(matches!(channels[3], Channel::Notification));
    }

    #[test]
    fn parse_channels_with_spaces() {
        let channels = Channel::parse_list(" console , signal ");
        assert_eq!(channels.len(), 2);
    }

    #[test]
    fn parse_channels_case_insensitive() {
        let channels = Channel::parse_list("CONSOLE,Signal,NOTIFICATION");
        assert_eq!(channels.len(), 3);
    }

    #[test]
    fn parse_channels_notifications_alias() {
        let channels = Channel::parse_list("notifications");
        assert_eq!(channels.len(), 1);
        assert!(matches!(channels[0], Channel::Notification));
    }

    #[test]
    fn parse_channels_unknown_ignored() {
        let channels = Channel::parse_list("console,fax,pigeon");
        assert_eq!(channels.len(), 1);
        assert!(matches!(channels[0], Channel::Console));
    }

    #[test]
    fn parse_channels_empty_string() {
        let channels = Channel::parse_list("");
        assert!(channels.is_empty());
    }

    // ── Rating parsing ──────────────────────────────────────────

    #[test]
    fn rating_from_str_words() {
        assert_eq!(Rating::from_str("again"), Some(Rating::Again));
        assert_eq!(Rating::from_str("hard"), Some(Rating::Hard));
        assert_eq!(Rating::from_str("good"), Some(Rating::Good));
        assert_eq!(Rating::from_str("easy"), Some(Rating::Easy));
    }

    #[test]
    fn rating_from_str_numbers() {
        assert_eq!(Rating::from_str("1"), Some(Rating::Again));
        assert_eq!(Rating::from_str("2"), Some(Rating::Hard));
        assert_eq!(Rating::from_str("3"), Some(Rating::Good));
        assert_eq!(Rating::from_str("4"), Some(Rating::Easy));
    }

    #[test]
    fn rating_from_str_case_insensitive() {
        assert_eq!(Rating::from_str("GOOD"), Some(Rating::Good));
        assert_eq!(Rating::from_str("Easy"), Some(Rating::Easy));
        assert_eq!(Rating::from_str("AGAIN"), Some(Rating::Again));
    }

    #[test]
    fn rating_from_str_with_whitespace() {
        assert_eq!(Rating::from_str("  good  "), Some(Rating::Good));
        assert_eq!(Rating::from_str("\t3\n"), Some(Rating::Good));
    }

    #[test]
    fn rating_from_str_invalid() {
        assert_eq!(Rating::from_str(""), None);
        assert_eq!(Rating::from_str("0"), None);
        assert_eq!(Rating::from_str("5"), None);
        assert_eq!(Rating::from_str("excellent"), None);
    }

    // ── Stub notifiers ──────────────────────────────────────────

    #[test]
    fn signal_notifier_returns_none() {
        let notifier = SignalNotifier;
        assert!(notifier.send("test", "body").is_none());
    }

    #[test]
    fn telegram_notifier_returns_none() {
        let notifier = TelegramNotifier;
        assert!(notifier.send("test", "body").is_none());
    }

    #[test]
    fn notification_center_non_interactive_returns_none() {
        let notifier = NotificationCenterNotifier {
            style: NotificationStyle::Notification,
            app: None,
        };
        // On non-macOS or without an app, this should return None
        let result = notifier.send("test", "body");
        assert!(result.is_none());
    }

    // ── build_notifiers ─────────────────────────────────────────

    #[test]
    fn build_notifiers_empty_channels_gives_console_fallback() {
        let notifiers = build_notifiers(&[]);
        assert_eq!(notifiers.len(), 1);
    }

    #[test]
    fn build_notifiers_respects_channel_list() {
        let channels = vec![Channel::Signal, Channel::Telegram];
        let notifiers = build_notifiers(&channels);
        assert_eq!(notifiers.len(), 2);
    }

    // ── escape_osascript (macOS only) ───────────────────────────

    #[cfg(target_os = "macos")]
    mod macos_tests {
        use super::*;

        #[test]
        fn escape_quotes() {
            assert_eq!(escape_osascript(r#"say "hello""#), r#"say \"hello\""#);
        }

        #[test]
        fn escape_backslash() {
            assert_eq!(escape_osascript(r"a\b"), r"a\\b");
        }

        #[test]
        fn escape_newlines() {
            assert_eq!(escape_osascript("line1\nline2"), r"line1\nline2");
        }

        #[test]
        fn escape_carriage_return_stripped() {
            assert_eq!(escape_osascript("hello\r\nworld"), r"hello\nworld");
        }

        #[test]
        fn escape_empty_string() {
            assert_eq!(escape_osascript(""), "");
        }

        #[test]
        fn escape_no_special_chars() {
            assert_eq!(escape_osascript("plain text"), "plain text");
        }
    }

    // ── Integration: review cycle ───────────────────────────────

    #[test]
    fn full_review_cycle_new_to_review() {
        let mut store = MemoryStore::sample(0);

        // First review: New -> Review
        store.review(0, Rating::Good, 0);
        assert_eq!(store.items[0].card.state, fsrs::State::Review);
        assert_eq!(store.items[0].card.reps, 1);

        // Second review at due time
        let due = store.items[0].due_at;
        store.review(0, Rating::Good, due);
        assert_eq!(store.items[0].card.state, fsrs::State::Review);
        assert_eq!(store.items[0].card.reps, 2);
    }

    #[test]
    fn full_review_cycle_lapse_and_recover() {
        let mut store = MemoryStore::sample(0);

        // Good review
        store.review(0, Rating::Good, 0);
        let due1 = store.items[0].due_at;

        // Lapse at due time
        store.review(0, Rating::Again, due1);
        assert_eq!(store.items[0].card.state, fsrs::State::Relearning);
        assert_eq!(store.items[0].card.lapses, 1);

        // Recover
        let due2 = store.items[0].due_at;
        store.review(0, Rating::Good, due2);
        assert_eq!(store.items[0].card.state, fsrs::State::Review);
    }

    #[test]
    fn all_ratings_produce_valid_intervals() {
        for rating in [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy] {
            let mut store = MemoryStore::sample(0);
            store.review(0, rating, 0);
            assert!(
                store.items[0].due_at > 0,
                "rating {:?} should produce due_at > 0",
                rating
            );
            assert!(
                store.items[0].card.stability > 0.0,
                "rating {:?} should produce positive stability",
                rating
            );
            assert!(
                store.items[0].card.difficulty >= 1.0 && store.items[0].card.difficulty <= 10.0,
                "rating {:?} difficulty={} out of range",
                rating,
                store.items[0].card.difficulty
            );
        }
    }
}
