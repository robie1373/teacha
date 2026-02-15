// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod fsrs;

use db::Database;
use fsrs::{CardState, Rating, State};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State as TauriState};

#[derive(Debug, Serialize, Deserialize)]
struct Card {
    id: Option<i64>,
    prompt: String,
    answer: String,
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
            prompt: db_card.prompt,
            answer: db_card.answer,
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
fn add_card(state: TauriState<AppState>, prompt: String, answer: String) -> Result<i64, String> {
    let db = state.db.lock().unwrap();
    db.add_card(&prompt, &answer).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_card(
    state: TauriState<AppState>,
    id: i64,
    prompt: String,
    answer: String,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.update_card(id, &prompt, &answer)
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

    // Calculate averages
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
    // Create or focus the review window
    if let Some(window) = app.get_webview_window("review") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        tauri::WebviewWindowBuilder::new(&app, "review", tauri::WebviewUrl::App("review.html".into()))
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
    // Initialize database
    let db = Database::new().expect("Failed to initialize database");

    // Add sample cards if empty
    if db.get_all_cards().unwrap_or_default().is_empty() {
        let _ = db.add_card(
            "Rust ownership rule?",
            "Each value has a single owner at a time.",
        );
        let _ = db.add_card(
            "HTTP 201 means?",
            "Resource created.",
        );
    }

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
            get_statistics,
            show_review_window,
            get_settings,
            update_settings,
        ])
        .setup(|app| {
            // Create system tray
            let _ = app.tray_by_id("main");
            
            // Open devtools in debug mode
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

    // ── Card Struct Tests ────────────────────────────────────────────

    #[test]
    fn card_struct_serializable() {
        let card = Card {
            id: Some(1),
            prompt: "Test?".to_string(),
            answer: "Answer".to_string(),
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

        let json = serde_json::to_string(&card);
        assert!(json.is_ok());
    }

    #[test]
    fn card_from_db_card_conversion() {
        let db = Database::test().expect("Failed to create database");
        let _id = db.add_card("Test?", "Answer").expect("Failed to add");
        
        let db_cards = db.get_all_cards().expect("Failed to get");
        let card = Card::from(db_cards[0].clone());
        
        assert_eq!(card.prompt, "Test?");
        assert_eq!(card.answer, "Answer");
        assert_eq!(card.state, State::New);
        assert!(card.id.is_some());
    }

    // ── Statistics Struct Tests ──────────────────────────────────────

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

        let json = serde_json::to_string(&stats);
        assert!(json.is_ok());
    }

    #[test]
    fn statistics_empty_optional_fields() {
        let stats = Statistics {
            total_cards: 0,
            cards_due: 0,
            cards_learning: 0,
            cards_review: 0,
            cards_new: 0,
            avg_difficulty: None,
            avg_stability: None,
        };

        let json = serde_json::to_string(&stats).expect("Failed to serialize");
        assert!(!json.contains("\"avg_difficulty\"") && !json.contains("\"avg_stability\""));
    }

    // ── AppSettings Struct Tests ─────────────────────────────────────

    #[test]
    fn app_settings_default() {
        let settings = AppSettings::default();
        assert_eq!(settings.notification_channel, "");
        assert!(!settings.signal_enabled);
        assert!(!settings.telegram_enabled);
    }

    // ── Rating Enum Tests ────────────────────────────────────────────

    #[test]
    fn rating_serializable() {
        let ratings = vec![Rating::Again, Rating::Hard, Rating::Good, Rating::Easy];
        
        for rating in ratings {
            let json = serde_json::to_string(&rating);
            assert!(json.is_ok());
        }
    }

    #[test]
    fn rating_mapping() {
        assert_eq!(Rating::Again as u8, 1);
        assert_eq!(Rating::Hard as u8, 2);
        assert_eq!(Rating::Good as u8, 3);
        assert_eq!(Rating::Easy as u8, 4);
    }

    // ── Database Integration Tests ───────────────────────────────────
    
    #[test]
    fn add_and_retrieve_cards() {
        let db = Database::test().expect("Failed to create database");
        
        let id1 = db.add_card("Q1?", "A1").expect("Failed to add 1");
        let id2 = db.add_card("Q2?", "A2").expect("Failed to add 2");
        
        let cards = db.get_all_cards().expect("Failed to get cards");
        
        assert_eq!(cards.len(), 2);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn update_and_retrieve_card() {
        let db = Database::test().expect("Failed to create database");
        let id = db.add_card("Original?", "Original").expect("Failed to add");
        
        db.update_card(id, "Updated?", "Updated").expect("Failed to update");
        
        let cards = db.get_all_cards().expect("Failed to get");
        assert_eq!(cards[0].prompt, "Updated?");
        assert_eq!(cards[0].answer, "Updated");
    }

    #[test]
    fn review_card_updates_state() {
        let db = Database::test().expect("Failed to create database");
        let id = db.add_card("Q?", "A").expect("Failed to add");
        
        let card_before = db.get_all_cards().expect("Failed to get")[0].fsrs_state.reps;
        
        db.review_card(id, Rating::Good).expect("Failed to review");
        
        let card_after = db.get_all_cards().expect("Failed to get")[0].fsrs_state.reps;
        
        assert_eq!(card_before, 0);
        assert_eq!(card_after, 1);
    }

    #[test]
    fn delete_card_removes_from_db() {
        let db = Database::test().expect("Failed to create database");
        let id = db.add_card("Q?", "A").expect("Failed to add");
        
        db.delete_card(id).expect("Failed to delete");
        
        let cards = db.get_all_cards().expect("Failed to get");
        assert_eq!(cards.len(), 0);
    }

    #[test]
    fn get_due_cards_includes_new() {
        let db = Database::test().expect("Failed to create database");
        let _ = db.add_card("Q?", "A").expect("Failed to add");
        
        let now = chrono::Utc::now().timestamp();
        let due = db.get_due_cards(now).expect("Failed to get due");
        
        assert_eq!(due.len(), 1);
    }

    // ── Settings Conversion Tests ────────────────────────────────────

    #[test]
    fn settings_to_json_structure() {
        let settings = AppSettings {
            notification_channel: "integrated".to_string(),
            signal_enabled: true,
            telegram_enabled: false,
        };

        let json = serde_json::json!({
            "pollSeconds": 60,
            "channels": {
                "integrated": settings.notification_channel == "integrated",
                "notification": settings.notification_channel == "notification",
                "signal": settings.signal_enabled,
                "telegram": settings.telegram_enabled
            }
        });

        assert_eq!(json["pollSeconds"].as_i64(), Some(60));
        assert_eq!(json["channels"]["signal"].as_bool(), Some(true));
    }

    // ── Complete Workflow Tests ──────────────────────────────────────

    #[test]
    fn complete_card_workflow() {
        let db = Database::test().expect("Failed to create database");
        
        // Add two cards
        let id1 = db.add_card("Rust?", "System language").expect("Failed to add 1");
        let id2 = db.add_card("Go?", "Concurrent language").expect("Failed to add 2");
        
        // Verify both are retrievable
        let all_cards = db.get_all_cards().expect("Failed to get all");
        assert_eq!(all_cards.len(), 2);
        
        // Verify both are due
        let now = chrono::Utc::now().timestamp();
        let due_cards = db.get_due_cards(now).expect("Failed to get due");
        assert_eq!(due_cards.len(), 2);
        
        // Review first card
        db.review_card(id1, Rating::Good).expect("Failed to review 1");
        
        // Verify state changed
        let updated = db.get_all_cards().expect("Failed to get after review");
        let card1 = updated.iter().find(|c| c.id == id1).expect("Card 1 not found");
        assert!(card1.fsrs_state.reps > 0);
        
        // Update second card
        db.update_card(id2, "Go (Updated)?", "Concurrent language").expect("Failed to update");
        
        let updated = db.get_all_cards().expect("Failed to get after update");
        let card2 = updated.iter().find(|c| c.id == id2).expect("Card 2 not found");
        assert_eq!(card2.prompt, "Go (Updated)?");
        
        // Delete first card
        db.delete_card(id1).expect("Failed to delete");
        
        let final_cards = db.get_all_cards().expect("Failed to get final");
        assert_eq!(final_cards.len(), 1);
        assert_eq!(final_cards[0].id, id2);
    }

    #[test]
    fn multiple_review_ratings() {
        let db = Database::test().expect("Failed to create database");
        
        let id_easy = db.add_card("Easy?", "A").expect("Failed to add");
        let id_good = db.add_card("Good?", "A").expect("Failed to add");
        let _id_hard = db.add_card("Hard?", "A").expect("Failed to add");
        let _id_again = db.add_card("Again?", "A").expect("Failed to add");
        
        db.review_card(id_easy, Rating::Easy).expect("Failed to review");
        db.review_card(id_good, Rating::Good).expect("Failed to review");
        
        let cards = db.get_all_cards().expect("Failed to get");
        
        let easy_card = cards.iter().find(|c| c.id == id_easy).expect("Not found");
        let good_card = cards.iter().find(|c| c.id == id_good).expect("Not found");
        
        assert!(easy_card.fsrs_state.stability > 0.0);
        assert!(good_card.fsrs_state.stability > 0.0);
    }
}

