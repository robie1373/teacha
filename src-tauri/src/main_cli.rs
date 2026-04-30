use std::env;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use teacha_core::db::{Database, DbCard};
use teacha_core::fsrs::Rating;
#[cfg(target_os = "macos")]
use std::process::Command as MacCommand;

const DEFAULT_POLL_SECS: u64 = 60;

// ── Notifier trait ──────────────────────────────────────────────────────────

trait Notifier {
    /// Fire the notification. Returns a rating if the channel supports
    /// interactive feedback; None for fire-and-forget channels.
    fn send(&self, title: &str, body: &str) -> Option<Rating>;
}

// ── Console ─────────────────────────────────────────────────────────────────

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

// ── Desktop (Linux: notify-send / dunst) ────────────────────────────────────

struct DesktopNotifier;

impl Notifier for DesktopNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        let _ = Command::new("notify-send")
            .arg("--app-name=Teacha")
            .arg(title)
            .arg(body)
            .status();
        None
    }
}

// ── ntfy (HTTP push) ────────────────────────────────────────────────────────

struct NtfyNotifier {
    url: String,
}

impl Notifier for NtfyNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        if let Err(e) = ureq::post(&self.url)
            .set("Title", title)
            .send_string(body)
        {
            eprintln!("[ntfy] failed to send: {e}");
        }
        None
    }
}

// ── Signal / Telegram stubs ──────────────────────────────────────────────────

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

// ── macOS Notification Center ────────────────────────────────────────────────

struct NotificationCenterNotifier;

impl Notifier for NotificationCenterNotifier {
    fn send(&self, title: &str, body: &str) -> Option<Rating> {
        #[cfg(target_os = "macos")]
        {
            return self.send_alert_with_rating(title, body);
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (title, body);
            None
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
        match MacCommand::new("osascript").arg("-e").arg(&script).output() {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout
                    .split(':')
                    .nth(1)
                    .and_then(|b| Rating::from_str(b.trim()))
                    .or(Some(Rating::Good))
            }
            Ok(_) => {
                eprintln!("[notification] alert dismissed or cancelled");
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

// ── Channel enum ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Channel {
    Console,
    Desktop,
    Ntfy,
    Signal,
    Telegram,
    Notification,
}

impl Channel {
    fn parse_list(raw: &str) -> Vec<Channel> {
        raw.split(',')
            .map(|v| v.trim().to_lowercase())
            .filter_map(|v| match v.as_str() {
                "console" => Some(Channel::Console),
                "desktop" => Some(Channel::Desktop),
                "ntfy" => Some(Channel::Ntfy),
                "signal" => Some(Channel::Signal),
                "telegram" => Some(Channel::Telegram),
                "notification" | "notifications" => Some(Channel::Notification),
                _ => None,
            })
            .collect()
    }
}

// ── Builder ──────────────────────────────────────────────────────────────────

fn build_notifiers(channels: &[Channel]) -> Vec<Box<dyn Notifier>> {
    let mut notifiers: Vec<Box<dyn Notifier>> = Vec::new();
    for channel in channels {
        match channel {
            Channel::Console => notifiers.push(Box::new(ConsoleNotifier)),
            Channel::Desktop => notifiers.push(Box::new(DesktopNotifier)),
            Channel::Ntfy => match read_ntfy_url() {
                Some(url) => notifiers.push(Box::new(NtfyNotifier { url })),
                None => eprintln!("[ntfy] TEACHA_NTFY_URL not set, skipping"),
            },
            Channel::Signal => notifiers.push(Box::new(SignalNotifier)),
            Channel::Telegram => notifiers.push(Box::new(TelegramNotifier)),
            Channel::Notification => notifiers.push(Box::new(NotificationCenterNotifier)),
        }
    }
    if notifiers.is_empty() {
        notifiers.push(Box::new(ConsoleNotifier));
    }
    notifiers
}

// ── Notification content ─────────────────────────────────────────────────────

/// Format the notification body from a card.
/// Tip cards (prompt is Some): show prompt then body.
/// Q&A cards (prompt is None): show body only.
fn card_notification_body(card: &DbCard) -> String {
    match &card.prompt {
        Some(prompt) => format!("{}\n\n{}", prompt, card.body),
        None => card.body.clone(),
    }
}

// ── Config ───────────────────────────────────────────────────────────────────

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn read_poll_seconds() -> u64 {
    env::var("TEACHA_POLL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_POLL_SECS)
}

fn read_channels() -> Vec<Channel> {
    env::var("TEACHA_CHANNELS")
        .ok()
        .map(|v| Channel::parse_list(&v))
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| {
            #[cfg(target_os = "macos")]
            { vec![Channel::Notification] }
            #[cfg(not(target_os = "macos"))]
            { vec![Channel::Desktop] }
        })
}

fn read_ntfy_url() -> Option<String> {
    env::var("TEACHA_NTFY_URL").ok().and_then(|url| {
        let trimmed = url.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    })
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let poll_seconds = read_poll_seconds();
    let channels = read_channels();
    let notifiers = build_notifiers(&channels);
    let db = Database::new().expect("Failed to open database");

    println!(
        "teacha daemon running. poll={}s channels={:?}",
        poll_seconds, channels
    );

    loop {
        let now = now_unix();
        let due_cards = db.get_due_cards(now).unwrap_or_default();

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

        thread::sleep(Duration::from_secs(poll_seconds));
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use teacha_core::fsrs::CardState;

    // ── Channel parsing ──────────────────────────────────────────────

    #[test]
    fn parse_console_channel() {
        let ch = Channel::parse_list("console");
        assert_eq!(ch.len(), 1);
        assert!(matches!(ch[0], Channel::Console));
    }

    #[test]
    fn parse_desktop_channel() {
        let ch = Channel::parse_list("desktop");
        assert_eq!(ch.len(), 1);
        assert!(matches!(ch[0], Channel::Desktop));
    }

    #[test]
    fn parse_ntfy_channel() {
        let ch = Channel::parse_list("ntfy");
        assert_eq!(ch.len(), 1);
        assert!(matches!(ch[0], Channel::Ntfy));
    }

    #[test]
    fn parse_multiple_channels() {
        let ch = Channel::parse_list("console,desktop,ntfy");
        assert_eq!(ch.len(), 3);
    }

    #[test]
    fn parse_channels_with_spaces() {
        let ch = Channel::parse_list(" console , desktop ");
        assert_eq!(ch.len(), 2);
    }

    #[test]
    fn parse_channels_case_insensitive() {
        let ch = Channel::parse_list("CONSOLE,Desktop,NTFY");
        assert_eq!(ch.len(), 3);
    }

    #[test]
    fn parse_notifications_alias() {
        let ch = Channel::parse_list("notifications");
        assert_eq!(ch.len(), 1);
        assert!(matches!(ch[0], Channel::Notification));
    }

    #[test]
    fn parse_channels_unknown_ignored() {
        let ch = Channel::parse_list("console,fax,pigeon");
        assert_eq!(ch.len(), 1);
        assert!(matches!(ch[0], Channel::Console));
    }

    #[test]
    fn parse_channels_empty_string() {
        assert!(Channel::parse_list("").is_empty());
    }

    // ── Rating parsing ───────────────────────────────────────────────

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

    // ── Notifiers ────────────────────────────────────────────────────

    #[test]
    fn signal_notifier_returns_none() {
        assert!(SignalNotifier.send("t", "b").is_none());
    }

    #[test]
    fn telegram_notifier_returns_none() {
        assert!(TelegramNotifier.send("t", "b").is_none());
    }

    #[test]
    fn desktop_notifier_returns_none() {
        // notify-send may not be present in test env; should not panic
        assert!(DesktopNotifier.send("t", "b").is_none());
    }

    #[test]
    fn notification_center_non_macos_returns_none() {
        #[cfg(not(target_os = "macos"))]
        assert!(NotificationCenterNotifier.send("t", "b").is_none());
    }

    // ── build_notifiers ──────────────────────────────────────────────

    #[test]
    fn build_notifiers_empty_falls_back_to_console() {
        assert_eq!(build_notifiers(&[]).len(), 1);
    }

    #[test]
    fn build_notifiers_console_only() {
        assert_eq!(build_notifiers(&[Channel::Console]).len(), 1);
    }

    #[test]
    fn build_notifiers_ntfy_without_url_falls_back_to_console() {
        env::remove_var("TEACHA_NTFY_URL");
        // ntfy skipped → empty → console fallback
        assert_eq!(build_notifiers(&[Channel::Ntfy]).len(), 1);
    }

    #[test]
    fn build_notifiers_signal_and_telegram() {
        let n = build_notifiers(&[Channel::Signal, Channel::Telegram]);
        assert_eq!(n.len(), 2);
    }

    // ── card_notification_body ───────────────────────────────────────

    fn tip_card() -> DbCard {
        DbCard {
            id: 1,
            title: "comma tip".to_string(),
            prompt: Some(", ffmpeg -i in.mp4 out.webm".to_string()),
            body: "Uses nix-index-database.".to_string(),
            tags: "nix,cli".to_string(),
            fsrs_state: CardState::new(),
            due_at: 0,
            created_at: 0,
        }
    }

    fn qa_card() -> DbCard {
        DbCard {
            id: 2,
            title: "HTTP 201?".to_string(),
            prompt: None,
            body: "Resource created.".to_string(),
            tags: "http".to_string(),
            fsrs_state: CardState::new(),
            due_at: 0,
            created_at: 0,
        }
    }

    #[test]
    fn tip_card_body_includes_prompt_and_body() {
        let body = card_notification_body(&tip_card());
        assert!(body.contains(", ffmpeg"));
        assert!(body.contains("nix-index-database"));
    }

    #[test]
    fn qa_card_body_is_body_only() {
        assert_eq!(card_notification_body(&qa_card()), "Resource created.");
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
        fn escape_backslash() {
            assert_eq!(escape_osascript(r"a\b"), r"a\\b");
        }

        #[test]
        fn escape_newlines() {
            assert_eq!(escape_osascript("a\nb"), r"a\nb");
        }

        #[test]
        fn escape_carriage_return_stripped() {
            assert_eq!(escape_osascript("a\r\nb"), r"a\nb");
        }
    }
}
