use std::env;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_POLL_SECS: u64 = 60;
const DEFAULT_INTERVAL_SECS: u64 = 60 * 60 * 24;

#[derive(Clone, Debug)]
struct MemoryItem {
    prompt: String,
    answer: String,
    interval_secs: u64,
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
                    interval_secs: DEFAULT_INTERVAL_SECS,
                    due_at: now,
                },
                MemoryItem {
                    prompt: "HTTP 201 means?".to_string(),
                    answer: "Resource created.".to_string(),
                    interval_secs: DEFAULT_INTERVAL_SECS,
                    due_at: now + 5,
                },
            ],
        }
    }

    fn due_items_mut(&mut self, now: u64) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| if item.due_at <= now { Some(index) } else { None })
            .collect()
    }

    fn reschedule(&mut self, index: usize, now: u64) {
        if let Some(item) = self.items.get_mut(index) {
            item.due_at = now + item.interval_secs;
        }
    }
}

trait Notifier {
    fn send(&self, title: &str, body: &str);
}

struct ConsoleNotifier;

impl Notifier for ConsoleNotifier {
    fn send(&self, title: &str, body: &str) {
        println!("[console] {title} - {body}");
    }
}

struct SignalNotifier;

impl Notifier for SignalNotifier {
    fn send(&self, title: &str, body: &str) {
        println!("[signal] {title} - {body}");
    }
}

struct TelegramNotifier;

impl Notifier for TelegramNotifier {
    fn send(&self, title: &str, body: &str) {
        println!("[telegram] {title} - {body}");
    }
}

struct NotificationCenterNotifier;

impl Notifier for NotificationCenterNotifier {
    fn send(&self, title: &str, body: &str) {
        println!("[notification] {title} - {body}");
    }
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
    let mut notifiers: Vec<Box<dyn Notifier>> = Vec::new();
    for channel in channels {
        match channel {
            Channel::Console => notifiers.push(Box::new(ConsoleNotifier)),
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
        .unwrap_or_else(|| vec![Channel::Console])
}

fn main() {
    let poll_seconds = read_poll_seconds();
    let channels = read_channels();
    let notifiers = build_notifiers(&channels);
    let mut store = MemoryStore::sample(now_unix());

    println!("teacha running. poll={}s channels={:?}", poll_seconds, channels);

    loop {
        let now = now_unix();
        let due_indices = store.due_items_mut(now);
        for index in due_indices {
            if let Some(item) = store.items.get(index) {
                let title = format!("Review: {}", item.prompt);
                let body = format!("Answer: {}", item.answer);
                for notifier in &notifiers {
                    notifier.send(&title, &body);
                }
            }
            store.reschedule(index, now);
        }

        thread::sleep(Duration::from_secs(poll_seconds));
    }
}
