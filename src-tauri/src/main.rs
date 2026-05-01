// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use teacha_core::db::{self, Database};
use teacha_core::fsrs::{Rating, State};
use teacha_core::seed::seed_if_empty;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State as TauriState};

#[derive(Debug, Serialize, Deserialize)]
struct Card {
    id: Option<i64>,
    title: String,
    prompt: Option<String>,
    body: String,
    tags: String,
    state: State,
    stability: f64,
    difficulty: f64,
    elapsed_days: f64,
    scheduled_days: f64,
    reps: u32,
    lapses: u32,
    last_review: f64,
    due_at: i64,
}

impl From<db::DbCard> for Card {
    fn from(db_card: db::DbCard) -> Self {
        Card {
            id: Some(db_card.id),
            title: db_card.title,
            prompt: db_card.prompt,
            body: db_card.body,
            tags: db_card.tags,
            state: db_card.fsrs_state.state,
            stability: db_card.fsrs_state.stability,
            difficulty: db_card.fsrs_state.difficulty,
            elapsed_days: db_card.fsrs_state.elapsed_days,
            scheduled_days: db_card.fsrs_state.scheduled_days,
            reps: db_card.fsrs_state.reps,
            lapses: db_card.fsrs_state.lapses,
            last_review: db_card.fsrs_state.last_review,
            due_at: db_card.due_at,
        }
    }
}

#[derive(Debug, Serialize)]
struct Statistics {
    total_cards: usize,
    cards_due: usize,
    cards_learning: usize,
    cards_review: usize,
    cards_new: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    avg_difficulty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avg_stability: Option<f64>,
}

#[derive(Default)]
struct AppSettings {
    notification_channel: String,
    signal_enabled: bool,
    telegram_enabled: bool,
}

struct AppState {
    db: Mutex<Database>,
    settings: Mutex<AppSettings>,
}

#[tauri::command]
fn get_all_cards(state: TauriState<AppState>) -> Result<Vec<Card>, String> {
    let db = state.db.lock().unwrap();
    db.get_all_cards()
        .map(|cards| cards.into_iter().map(Card::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_due_cards(state: TauriState<AppState>) -> Result<Vec<Card>, String> {
    let db = state.db.lock().unwrap();
    let now = chrono::Utc::now().timestamp();
    db.get_due_cards(now)
        .map(|cards| cards.into_iter().map(Card::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_due_card(state: TauriState<AppState>) -> Result<Option<Card>, String> {
    let db = state.db.lock().unwrap();
    let now = chrono::Utc::now().timestamp();
    db.get_due_cards(now)
        .map(|mut cards| cards.pop().map(Card::from))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn add_card(
    state: TauriState<AppState>,
    title: String,
    prompt: Option<String>,
    body: String,
    tags: String,
) -> Result<i64, String> {
    let db = state.db.lock().unwrap();
    db.add_card(&title, prompt.as_deref(), &body, &tags)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_card(
    state: TauriState<AppState>,
    id: i64,
    title: String,
    prompt: Option<String>,
    body: String,
    tags: String,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.update_card(id, &title, prompt.as_deref(), &body, &tags)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_card(state: TauriState<AppState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.delete_card(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn review_card(state: TauriState<AppState>, id: i64, rating: u8) -> Result<Card, String> {
    let rating = match rating {
        1 => Rating::Again,
        2 => Rating::Hard,
        3 => Rating::Good,
        4 => Rating::Easy,
        _ => return Err("Invalid rating".to_string()),
    };
    let db = state.db.lock().unwrap();
    db.review_card(id, rating)
        .map(Card::from)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_cards_by_tag(state: TauriState<AppState>, tag: String) -> Result<Vec<Card>, String> {
    let db = state.db.lock().unwrap();
    db.get_cards_by_tag(&tag)
        .map(|cards| cards.into_iter().map(Card::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_statistics(state: TauriState<AppState>) -> Result<Statistics, String> {
    let db = state.db.lock().unwrap();
    let all_cards = db.get_all_cards().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let due_cards = db.get_due_cards(now).map_err(|e| e.to_string())?;

    let cards_learning = all_cards
        .iter()
        .filter(|c| c.fsrs_state.state == State::Learning)
        .count();
    let cards_review = all_cards
        .iter()
        .filter(|c| c.fsrs_state.state == State::Review)
        .count();
    let cards_new = all_cards
        .iter()
        .filter(|c| c.fsrs_state.state == State::New)
        .count();

    let reviewed_cards: Vec<_> = all_cards
        .iter()
        .filter(|c| c.fsrs_state.reps > 0)
        .collect();

    let avg_difficulty = if reviewed_cards.is_empty() {
        None
    } else {
        let sum: f64 = reviewed_cards.iter().map(|c| c.fsrs_state.difficulty).sum();
        Some(sum / reviewed_cards.len() as f64)
    };

    let avg_stability = if reviewed_cards.is_empty() {
        None
    } else {
        let sum: f64 = reviewed_cards.iter().map(|c| c.fsrs_state.stability).sum();
        Some(sum / reviewed_cards.len() as f64)
    };

    Ok(Statistics {
        total_cards: all_cards.len(),
        cards_due: due_cards.len(),
        cards_learning,
        cards_review,
        cards_new,
        avg_difficulty,
        avg_stability,
    })
}

#[tauri::command]
fn show_review_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("review") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        tauri::WebviewWindowBuilder::new(
            &app,
            "review",
            tauri::WebviewUrl::App("review.html".into()),
        )
        .title("Review Card")
        .inner_size(500.0, 400.0)
        .resizable(false)
        .center()
        .build()
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_settings(state: TauriState<AppState>) -> Result<serde_json::Value, String> {
    let settings = state.settings.lock().unwrap();
    Ok(serde_json::json!({
        "pollSeconds": 60,
        "channels": {
            "integrated": settings.notification_channel == "integrated",
            "notification": settings.notification_channel == "notification",
            "signal": settings.signal_enabled,
            "telegram": settings.telegram_enabled
        }
    }))
}

#[tauri::command]
fn update_settings(
    state: TauriState<AppState>,
    notification_channel: Option<String>,
    signal_enabled: Option<bool>,
    telegram_enabled: Option<bool>,
) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    if let Some(channel) = notification_channel {
        settings.notification_channel = channel;
    }
    if let Some(enabled) = signal_enabled {
        settings.signal_enabled = enabled;
    }
    if let Some(enabled) = telegram_enabled {
        settings.telegram_enabled = enabled;
    }
    Ok(())
}

fn main() {
    let db = Database::new().expect("Failed to initialize database");

    seed_if_empty(&db);

    let app_state = AppState {
        db: Mutex::new(db),
        settings: Mutex::new(AppSettings {
            notification_channel: "integrated".to_string(),
            signal_enabled: false,
            telegram_enabled: false,
        }),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_all_cards,
            get_due_cards,
            get_due_card,
            add_card,
            update_card,
            delete_card,
            review_card,
            get_cards_by_tag,
            get_statistics,
            show_review_window,
            get_settings,
            update_settings,
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Card struct ──────────────────────────────────────────────────

    #[test]
    fn card_struct_serializable() {
        let card = Card {
            id: Some(1),
            title: "Test tip".to_string(),
            prompt: Some("cmd".to_string()),
            body: "Explanation".to_string(),
            tags: "cli".to_string(),
            state: State::Review,
            stability: 2.5,
            difficulty: 3.0,
            elapsed_days: 5.0,
            scheduled_days: 7.0,
            reps: 10,
            lapses: 1,
            last_review: 0.0,
            due_at: 1000,
        };
        assert!(serde_json::to_string(&card).is_ok());
    }

    #[test]
    fn card_struct_serializable_without_prompt() {
        let card = Card {
            id: Some(2),
            title: "HTTP 201?".to_string(),
            prompt: None,
            body: "Resource created.".to_string(),
            tags: "http".to_string(),
            state: State::New,
            stability: 0.0,
            difficulty: 0.0,
            elapsed_days: 0.0,
            scheduled_days: 0.0,
            reps: 0,
            lapses: 0,
            last_review: 0.0,
            due_at: 0,
        };
        let json = serde_json::to_string(&card).expect("serialize");
        // prompt: null must appear so the frontend can distinguish tip vs Q&A
        assert!(json.contains("\"prompt\":null"));
    }

    #[test]
    fn card_from_db_card_tip() {
        let db = Database::open_in_memory().expect("test db");
        let id = db
            .add_card("comma tip", Some(", cmd"), "Details", "nix,cli")
            .expect("add");
        let db_cards = db.get_all_cards().expect("get");
        let card = Card::from(db_cards.into_iter().find(|c| c.id == id).unwrap());
        assert_eq!(card.title, "comma tip");
        assert_eq!(card.prompt, Some(", cmd".to_string()));
        assert_eq!(card.body, "Details");
        assert_eq!(card.tags, "nix,cli");
        assert_eq!(card.state, State::New);
        assert!(card.id.is_some());
    }

    #[test]
    fn card_from_db_card_qa() {
        let db = Database::open_in_memory().expect("test db");
        let id = db
            .add_card("HTTP 201?", None, "Resource created.", "http")
            .expect("add");
        let db_cards = db.get_all_cards().expect("get");
        let card = Card::from(db_cards.into_iter().find(|c| c.id == id).unwrap());
        assert_eq!(card.prompt, None);
    }

    // ── Statistics ───────────────────────────────────────────────────

    #[test]
    fn statistics_struct_serializable() {
        let stats = Statistics {
            total_cards: 10,
            cards_due: 3,
            cards_learning: 2,
            cards_review: 5,
            cards_new: 2,
            avg_difficulty: Some(3.14),
            avg_stability: Some(2.0),
        };
        assert!(serde_json::to_string(&stats).is_ok());
    }

    #[test]
    fn statistics_omits_empty_averages() {
        let stats = Statistics {
            total_cards: 0,
            cards_due: 0,
            cards_learning: 0,
            cards_review: 0,
            cards_new: 0,
            avg_difficulty: None,
            avg_stability: None,
        };
        let json = serde_json::to_string(&stats).expect("serialize");
        assert!(!json.contains("avg_difficulty"));
        assert!(!json.contains("avg_stability"));
    }

    // ── AppSettings ──────────────────────────────────────────────────

    #[test]
    fn app_settings_default() {
        let settings = AppSettings::default();
        assert_eq!(settings.notification_channel, "");
        assert!(!settings.signal_enabled);
        assert!(!settings.telegram_enabled);
    }

    // ── Rating mapping ───────────────────────────────────────────────

    #[test]
    fn rating_mapping() {
        assert_eq!(Rating::Again as u8, 1);
        assert_eq!(Rating::Hard as u8, 2);
        assert_eq!(Rating::Good as u8, 3);
        assert_eq!(Rating::Easy as u8, 4);
    }

    // ── Integration ──────────────────────────────────────────────────

    #[test]
    fn add_and_retrieve_mixed_cards() {
        let db = Database::open_in_memory().expect("test db");
        db.add_card("comma tip", Some(", ffmpeg ..."), "Details", "nix")
            .expect("add tip");
        db.add_card("HTTP 201?", None, "Resource created.", "http")
            .expect("add qa");
        assert_eq!(db.get_all_cards().expect("get").len(), 2);
    }

    #[test]
    fn complete_card_workflow() {
        let db = Database::open_in_memory().expect("test db");

        let id1 = db
            .add_card("Tip 1", Some("cmd1"), "Body 1", "cli")
            .expect("add 1");
        let id2 = db.add_card("Fact", None, "Answer", "").expect("add 2");

        assert_eq!(db.get_all_cards().expect("get").len(), 2);

        let now = chrono::Utc::now().timestamp();
        assert_eq!(db.get_due_cards(now).expect("due").len(), 2);

        db.review_card(id1, Rating::Good).expect("review");
        let updated = db.get_all_cards().expect("get after review");
        let card1 = updated.iter().find(|c| c.id == id1).unwrap();
        assert!(card1.fsrs_state.reps > 0);

        db.update_card(id2, "Fact (updated)", None, "Better answer", "http")
            .expect("update");
        let updated2 = db.get_all_cards().expect("get after update");
        let card2 = updated2.iter().find(|c| c.id == id2).unwrap();
        assert_eq!(card2.title, "Fact (updated)");

        db.delete_card(id1).expect("delete");
        assert_eq!(db.get_all_cards().expect("final get").len(), 1);
    }
}
