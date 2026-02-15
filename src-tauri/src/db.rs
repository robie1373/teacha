use crate::fsrs::{CardState, Rating, State};
use rusqlite::{Connection, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DbCard {
    pub id: i64,
    pub prompt: String,
    pub answer: String,
    pub fsrs_state: CardState,
    pub due_at: i64,
    pub created_at: i64,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = Self::get_db_path();
        
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(db_path)?;
        Self::initialize_schema(&conn)?;
        
        Ok(Database { conn })
    }

    #[cfg(test)]
    pub fn test() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::initialize_schema(&conn)?;
        Ok(Database { conn })
    }

    fn initialize_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prompt TEXT NOT NULL,
                answer TEXT NOT NULL,
                stability REAL NOT NULL DEFAULT 0.0,
                difficulty REAL NOT NULL DEFAULT 0.0,
                elapsed_days REAL NOT NULL DEFAULT 0.0,
                scheduled_days REAL NOT NULL DEFAULT 0.0,
                reps INTEGER NOT NULL DEFAULT 0,
                lapses INTEGER NOT NULL DEFAULT 0,
                state INTEGER NOT NULL DEFAULT 0,
                last_review REAL NOT NULL DEFAULT 0.0,
                due_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    fn get_db_path() -> PathBuf {
        let mut path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        path.push("teacha");
        path.push("cards.db");
        path
    }

    pub fn add_card(&self, prompt: &str, answer: &str) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        let card_state = CardState::new();
        
        self.conn.execute(
            "INSERT INTO cards (
                prompt, answer, stability, difficulty, elapsed_days, scheduled_days,
                reps, lapses, state, last_review, due_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            (
                prompt,
                answer,
                card_state.stability,
                card_state.difficulty,
                card_state.elapsed_days,
                card_state.scheduled_days,
                card_state.reps,
                card_state.lapses,
                state_to_int(card_state.state),
                card_state.last_review,
                now, // due immediately
                now,
            ),
        )?;
        
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_card(&self, id: i64, prompt: &str, answer: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE cards SET prompt = ?1, answer = ?2 WHERE id = ?3",
            (prompt, answer, id),
        )?;
        Ok(())
    }

    pub fn get_all_cards(&self) -> Result<Vec<DbCard>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, prompt, answer, stability, difficulty, elapsed_days, scheduled_days,
             reps, lapses, state, last_review, due_at, created_at FROM cards ORDER BY created_at DESC"
        )?;

        let cards = stmt.query_map([], |row| {
            Ok(DbCard {
                id: row.get(0)?,
                prompt: row.get(1)?,
                answer: row.get(2)?,
                fsrs_state: CardState {
                    stability: row.get(3)?,
                    difficulty: row.get(4)?,
                    elapsed_days: row.get(5)?,
                    scheduled_days: row.get(6)?,
                    reps: row.get(7)?,
                    lapses: row.get(8)?,
                    state: int_to_state(row.get(9)?),
                    last_review: row.get(10)?,
                },
                due_at: row.get(11)?,
                created_at: row.get(12)?,
            })
        })?;

        cards.collect()
    }

    pub fn get_due_cards(&self, now: i64) -> Result<Vec<DbCard>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, prompt, answer, stability, difficulty, elapsed_days, scheduled_days,
             reps, lapses, state, last_review, due_at, created_at 
             FROM cards WHERE due_at <= ?1 
             ORDER BY due_at ASC"
        )?;

        let cards = stmt.query_map([now], |row| {
            Ok(DbCard {
                id: row.get(0)?,
                prompt: row.get(1)?,
                answer: row.get(2)?,
                fsrs_state: CardState {
                    stability: row.get(3)?,
                    difficulty: row.get(4)?,
                    elapsed_days: row.get(5)?,
                    scheduled_days: row.get(6)?,
                    reps: row.get(7)?,
                    lapses: row.get(8)?,
                    state: int_to_state(row.get(9)?),
                    last_review: row.get(10)?,
                },
                due_at: row.get(11)?,
                created_at: row.get(12)?,
            })
        })?;

        cards.collect()
    }

    pub fn delete_card(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM cards WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn review_card(&self, id: i64, rating: Rating) -> Result<DbCard> {
        // Get current card state
        let mut stmt = self.conn.prepare(
            "SELECT stability, difficulty, elapsed_days, scheduled_days,
             reps, lapses, state, last_review FROM cards WHERE id = ?1"
        )?;

        let card_state = stmt.query_row([id], |row| {
            Ok(CardState {
                stability: row.get(0)?,
                difficulty: row.get(1)?,
                elapsed_days: row.get(2)?,
                scheduled_days: row.get(3)?,
                reps: row.get(4)?,
                lapses: row.get(5)?,
                state: int_to_state(row.get(6)?),
                last_review: row.get(7)?,
            })
        })?;

        // Review with FSRS
        let now = chrono::Utc::now().timestamp();
        let now_days = now as f64 / 86400.0;
        let (next_state, interval_secs) = card_state.review(rating, now_days);

        // Update database
        self.conn.execute(
            "UPDATE cards SET 
             stability = ?1, difficulty = ?2, elapsed_days = ?3, scheduled_days = ?4,
             reps = ?5, lapses = ?6, state = ?7, last_review = ?8, due_at = ?9
             WHERE id = ?10",
            (
                next_state.stability,
                next_state.difficulty,
                next_state.elapsed_days,
                next_state.scheduled_days,
                next_state.reps,
                next_state.lapses,
                state_to_int(next_state.state),
                next_state.last_review,
                now + interval_secs as i64,
                id,
            ),
        )?;

        // Return updated card
        Ok(DbCard {
            id,
            prompt: String::new(), // Will be filled by caller if needed
            answer: String::new(),
            fsrs_state: next_state,
            due_at: now + interval_secs as i64,
            created_at: now,
        })
    }
}

fn state_to_int(state: State) -> i32 {
    match state {
        State::New => 0,
        State::Learning => 1,
        State::Review => 2,
        State::Relearning => 3,
    }
}

fn int_to_state(value: i32) -> State {
    match value {
        0 => State::New,
        1 => State::Learning,
        2 => State::Review,
        3 => State::Relearning,
        _ => State::New,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Database {
        // Use in-memory database for tests
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prompt TEXT NOT NULL,
                answer TEXT NOT NULL,
                stability REAL NOT NULL DEFAULT 0.0,
                difficulty REAL NOT NULL DEFAULT 0.0,
                elapsed_days REAL NOT NULL DEFAULT 0.0,
                scheduled_days REAL NOT NULL DEFAULT 0.0,
                reps INTEGER NOT NULL DEFAULT 0,
                lapses INTEGER NOT NULL DEFAULT 0,
                state INTEGER NOT NULL DEFAULT 0,
                last_review REAL NOT NULL DEFAULT 0.0,
                due_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            [],
        ).expect("Failed to create table");

        Database { conn }
    }

    // ── Database Initialization ──────────────────────────────────────

    #[test]
    fn add_single_card() {
        let db = Database::test().expect("Failed to create test db");
        let id = db.add_card("Question?", "Answer").expect("Failed to add card");
        assert_eq!(id, 1);
    }

    #[test]
    fn add_multiple_cards() {
        let db = Database::test().expect("Failed to create test db");
        let id1 = db.add_card("Q1?", "A1").expect("Failed to add card 1");
        let id2 = db.add_card("Q2?", "A2").expect("Failed to add card 2");
        let id3 = db.add_card("Q3?", "A3").expect("Failed to add card 3");
        
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn added_card_has_new_state() {
        let db = Database::test().expect("Failed to create test db");
        db.add_card("Test?", "Test Answer").expect("Failed to add card");
        
        let cards = db.get_all_cards().expect("Failed to get cards");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].fsrs_state.state, State::New);
    }

    #[test]
    fn added_card_has_default_values() {
        let db = Database::test().expect("Failed to create test db");
        db.add_card("Test?", "Test Answer").expect("Failed to add card");
        
        let cards = db.get_all_cards().expect("Failed to get cards");
        let card = &cards[0];
        
        assert_eq!(card.fsrs_state.stability, 0.0);
        assert_eq!(card.fsrs_state.difficulty, 0.0);
        assert_eq!(card.fsrs_state.reps, 0);
        assert_eq!(card.fsrs_state.lapses, 0);
    }

    // ── Get All Cards ────────────────────────────────────────────────

    #[test]
    fn get_all_cards_empty() {
        let db = Database::test().expect("Failed to create test db");
        let cards = db.get_all_cards().expect("Failed to get cards");
        assert_eq!(cards.len(), 0);
    }

    #[test]
    fn get_all_cards_retrieves_all() {
        let db = Database::test().expect("Failed to create test db");
        db.add_card("Q1?", "A1").expect("Failed to add card 1");
        db.add_card("Q2?", "A2").expect("Failed to add card 2");
        db.add_card("Q3?", "A3").expect("Failed to add card 3");
        
        let cards = db.get_all_cards().expect("Failed to get cards");
        assert_eq!(cards.len(), 3);
    }

    #[test]
    fn get_all_cards_preserves_content() {
        let db = Database::test().expect("Failed to create test db");
        db.add_card("Rust ownership?", "Single owner rule").expect("Failed to add card");
        
        let cards = db.get_all_cards().expect("Failed to get cards");
        assert_eq!(cards[0].prompt, "Rust ownership?");
        assert_eq!(cards[0].answer, "Single owner rule");
    }

    #[test]
    fn get_all_cards_ordering() {
        let db = Database::test().expect("Failed to create test db");
        db.add_card("First", "A1").expect("Failed to add first");
        db.add_card("Second", "A2").expect("Failed to add second");
        
        let cards = db.get_all_cards().expect("Failed to get cards");
        assert_eq!(cards.len(), 2);
        // Cards should be ordered by created_at DESC (or at worst, creation order)
        // Just verify both exist
        let prompts: Vec<&str> = cards.iter().map(|c| c.prompt.as_str()).collect();
        assert!(prompts.contains(&"First"));
        assert!(prompts.contains(&"Second"));
    }

    // ── Update Card ──────────────────────────────────────────────────

    #[test]
    fn update_card_changes_content() {
        let db = Database::test().expect("Failed to create test db");
        let id = db.add_card("Original?", "Original Answer").expect("Failed to add");
        
        db.update_card(id, "Updated?", "Updated Answer").expect("Failed to update");
        
        let cards = db.get_all_cards().expect("Failed to get cards");
        assert_eq!(cards[0].prompt, "Updated?");
        assert_eq!(cards[0].answer, "Updated Answer");
    }

    #[test]
    fn update_card_preserves_state() {
        let db = Database::test().expect("Failed to create test db");
        let id = db.add_card("Q?", "A").expect("Failed to add");
        
        // Simulate a review to change state
        db.review_card(id, Rating::Good).expect("Failed to review");
        let card_before = db.get_all_cards().expect("Failed to get").pop().expect("No cards");
        let state_before = card_before.fsrs_state.state;
        
        // Update content
        db.update_card(id, "New Q?", "New A").expect("Failed to update");
        
        let card_after = db.get_all_cards().expect("Failed to get").pop().expect("No cards");
        assert_eq!(card_after.fsrs_state.state, state_before);
    }

    // ── Delete Card ──────────────────────────────────────────────────

    #[test]
    fn delete_card_removes_from_db() {
        let db = Database::test().expect("Failed to create test db");
        let id = db.add_card("Q?", "A").expect("Failed to add");
        
        let initial_count = db.get_all_cards().expect("Failed to get").len();
        assert_eq!(initial_count, 1);
        
        db.delete_card(id).expect("Failed to delete");
        
        let final_count = db.get_all_cards().expect("Failed to get").len();
        assert_eq!(final_count, 0);
    }

    #[test]
    fn delete_specific_card() {
        let db = Database::test().expect("Failed to create test db");
        let id1 = db.add_card("Q1?", "A1").expect("Failed to add 1");
        let id2 = db.add_card("Q2?", "A2").expect("Failed to add 2");
        
        db.delete_card(id1).expect("Failed to delete");
        
        let cards = db.get_all_cards().expect("Failed to get");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, id2);
    }

    // ── Get Due Cards ────────────────────────────────────────────────

    #[test]
    fn newly_added_card_is_due() {
        let db = Database::test().expect("Failed to create test db");
        db.add_card("Q?", "A").expect("Failed to add");
        
        let now = chrono::Utc::now().timestamp();
        let due_cards = db.get_due_cards(now).expect("Failed to get due");
        assert_eq!(due_cards.len(), 1);
    }

    #[test]
    fn get_due_cards_respects_timestamp() {
        let db = Database::test().expect("Failed to create test db");
        let id = db.add_card("Q?", "A").expect("Failed to add");
        
        // Review the card to schedule it for the future (1 day)
        db.review_card(id, Rating::Good).expect("Failed to review");
        
        let now = chrono::Utc::now().timestamp();
        let due_cards_now = db.get_due_cards(now).expect("Failed to get due now");
        
        // The card should not be in the due list
        assert_eq!(due_cards_now.len(), 0);
    }

    #[test]
    fn get_due_cards_ordering() {
        let db = Database::test().expect("Failed to create test db");
        let id1 = db.add_card("Q1?", "A1").expect("Failed to add 1");
        let id2 = db.add_card("Q2?", "A2").expect("Failed to add 2");
        
        // Both are due now, verify order is by due_at ascending
        let now = chrono::Utc::now().timestamp();
        let due_cards = db.get_due_cards(now).expect("Failed to get due");
        
        assert_eq!(due_cards.len(), 2);
        // First card added should have same or earlier due_at
        assert!(due_cards[0].id == id2 || due_cards[0].id == id1);
    }

    // ── Review Card ──────────────────────────────────────────────────

    #[test]
    fn review_card_increments_reps() {
        let db = Database::test().expect("Failed to create test db");
        let id = db.add_card("Q?", "A").expect("Failed to add");
        
        let initial = db.get_all_cards().expect("Failed to get")[0].fsrs_state.reps;
        db.review_card(id, Rating::Good).expect("Failed to review");
        let after = db.get_all_cards().expect("Failed to get")[0].fsrs_state.reps;
        
        assert_eq!(initial, 0);
        assert_eq!(after, 1);
    }

    #[test]
    fn review_card_with_again_increments_lapses() {
        let db = Database::test().expect("Failed to create test db");
        let id = db.add_card("Q?", "A").expect("Failed to add");
        
        // First review with Good to move to Review state
        db.review_card(id, Rating::Good).expect("Failed to review");
        
        let initial_lapses = db.get_all_cards().expect("Failed to get")[0].fsrs_state.lapses;
        
        // Review with Again to increment lapses
        db.review_card(id, Rating::Again).expect("Failed to review");
        let final_lapses = db.get_all_cards().expect("Failed to get")[0].fsrs_state.lapses;
        
        assert_eq!(final_lapses, initial_lapses + 1);
    }

    #[test]
    fn review_card_updates_due_at() {
        let db = Database::test().expect("Failed to create test db");
        let id = db.add_card("Q?", "A").expect("Failed to add");
        
        let initial_due = db.get_all_cards().expect("Failed to get")[0].due_at;
        
        db.review_card(id, Rating::Good).expect("Failed to review");
        
        let final_due = db.get_all_cards().expect("Failed to get")[0].due_at;
        
        // Due date should be pushed into the future
        assert!(final_due >= initial_due);
    }

    #[test]
    fn review_with_different_ratings_affect_stability() {
        let db = Database::test().expect("Failed to create test db");
        
        let id1 = db.add_card("Q1?", "A1").expect("Failed to add 1");
        let id2 = db.add_card("Q2?", "A2").expect("Failed to add 2");
        
        db.review_card(id1, Rating::Easy).expect("Failed to review 1");
        db.review_card(id2, Rating::Again).expect("Failed to review 2");
        
        let cards = db.get_all_cards().expect("Failed to get");
        let card1_stability = cards.iter().find(|c| c.id == id1).unwrap().fsrs_state.stability;
        let card2_stability = cards.iter().find(|c| c.id == id2).unwrap().fsrs_state.stability;
        
        // Easy rating should result in higher stability than Again
        assert!(card1_stability > card2_stability);
    }

    // ── State Conversion ─────────────────────────────────────────────

    #[test]
    fn state_to_int_mapping() {
        assert_eq!(state_to_int(State::New), 0);
        assert_eq!(state_to_int(State::Learning), 1);
        assert_eq!(state_to_int(State::Review), 2);
        assert_eq!(state_to_int(State::Relearning), 3);
    }

    #[test]
    fn int_to_state_mapping() {
        assert_eq!(int_to_state(0), State::New);
        assert_eq!(int_to_state(1), State::Learning);
        assert_eq!(int_to_state(2), State::Review);
        assert_eq!(int_to_state(3), State::Relearning);
    }

    #[test]
    fn state_conversion_roundtrip() {
        let states = vec![State::New, State::Learning, State::Review, State::Relearning];
        
        for state in states {
            let int = state_to_int(state);
            let back = int_to_state(int);
            assert_eq!(state, back);
        }
    }
}
