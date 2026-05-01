use crate::fsrs::{CardState, Rating, State};
use rusqlite::{Connection, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DbCard {
    pub id: i64,
    pub title: String,
    pub prompt: Option<String>,
    pub body: String,
    pub tags: String,
    pub fsrs_state: CardState,
    pub due_at: i64,
    #[allow(dead_code)]
    pub created_at: i64,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = Self::get_db_path();

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(db_path)?;
        Self::initialize_schema(&conn)?;

        Ok(Database { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::initialize_schema(&conn)?;
        Ok(Database { conn })
    }

    fn initialize_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                title          TEXT NOT NULL,
                prompt         TEXT,
                body           TEXT NOT NULL,
                tags           TEXT NOT NULL DEFAULT '',
                stability      REAL NOT NULL DEFAULT 0.0,
                difficulty     REAL NOT NULL DEFAULT 0.0,
                elapsed_days   REAL NOT NULL DEFAULT 0.0,
                scheduled_days REAL NOT NULL DEFAULT 0.0,
                reps           INTEGER NOT NULL DEFAULT 0,
                lapses         INTEGER NOT NULL DEFAULT 0,
                state          INTEGER NOT NULL DEFAULT 0,
                last_review    REAL NOT NULL DEFAULT 0.0,
                due_at         INTEGER NOT NULL,
                created_at     INTEGER NOT NULL
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

    pub fn add_card(
        &self,
        title: &str,
        prompt: Option<&str>,
        body: &str,
        tags: &str,
    ) -> Result<i64> {
        let now = chrono::Utc::now().timestamp();
        let card_state = CardState::new();

        self.conn.execute(
            "INSERT INTO cards (
                title, prompt, body, tags,
                stability, difficulty, elapsed_days, scheduled_days,
                reps, lapses, state, last_review, due_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            (
                title,
                prompt,
                body,
                tags,
                card_state.stability,
                card_state.difficulty,
                card_state.elapsed_days,
                card_state.scheduled_days,
                card_state.reps,
                card_state.lapses,
                state_to_int(card_state.state),
                card_state.last_review,
                now,
                now,
            ),
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_card(
        &self,
        id: i64,
        title: &str,
        prompt: Option<&str>,
        body: &str,
        tags: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE cards SET title = ?1, prompt = ?2, body = ?3, tags = ?4 WHERE id = ?5",
            (title, prompt, body, tags, id),
        )?;
        Ok(())
    }

    pub fn get_card(&self, id: i64) -> Result<DbCard> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, prompt, body, tags,
             stability, difficulty, elapsed_days, scheduled_days,
             reps, lapses, state, last_review, due_at, created_at
             FROM cards WHERE id = ?1",
        )?;
        stmt.query_row([id], |row| {
            Ok(DbCard {
                id: row.get(0)?,
                title: row.get(1)?,
                prompt: row.get(2)?,
                body: row.get(3)?,
                tags: row.get(4)?,
                fsrs_state: CardState {
                    stability: row.get(5)?,
                    difficulty: row.get(6)?,
                    elapsed_days: row.get(7)?,
                    scheduled_days: row.get(8)?,
                    reps: row.get(9)?,
                    lapses: row.get(10)?,
                    state: int_to_state(row.get(11)?),
                    last_review: row.get(12)?,
                },
                due_at: row.get(13)?,
                created_at: row.get(14)?,
            })
        })
    }

    pub fn get_all_cards(&self) -> Result<Vec<DbCard>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, prompt, body, tags,
             stability, difficulty, elapsed_days, scheduled_days,
             reps, lapses, state, last_review, due_at, created_at
             FROM cards ORDER BY created_at DESC",
        )?;

        let cards = stmt.query_map([], |row| {
            Ok(DbCard {
                id: row.get(0)?,
                title: row.get(1)?,
                prompt: row.get(2)?,
                body: row.get(3)?,
                tags: row.get(4)?,
                fsrs_state: CardState {
                    stability: row.get(5)?,
                    difficulty: row.get(6)?,
                    elapsed_days: row.get(7)?,
                    scheduled_days: row.get(8)?,
                    reps: row.get(9)?,
                    lapses: row.get(10)?,
                    state: int_to_state(row.get(11)?),
                    last_review: row.get(12)?,
                },
                due_at: row.get(13)?,
                created_at: row.get(14)?,
            })
        })?;

        cards.collect()
    }

    pub fn get_due_cards(&self, now: i64) -> Result<Vec<DbCard>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, prompt, body, tags,
             stability, difficulty, elapsed_days, scheduled_days,
             reps, lapses, state, last_review, due_at, created_at
             FROM cards WHERE due_at <= ?1
             ORDER BY due_at ASC",
        )?;

        let cards = stmt.query_map([now], |row| {
            Ok(DbCard {
                id: row.get(0)?,
                title: row.get(1)?,
                prompt: row.get(2)?,
                body: row.get(3)?,
                tags: row.get(4)?,
                fsrs_state: CardState {
                    stability: row.get(5)?,
                    difficulty: row.get(6)?,
                    elapsed_days: row.get(7)?,
                    scheduled_days: row.get(8)?,
                    reps: row.get(9)?,
                    lapses: row.get(10)?,
                    state: int_to_state(row.get(11)?),
                    last_review: row.get(12)?,
                },
                due_at: row.get(13)?,
                created_at: row.get(14)?,
            })
        })?;

        cards.collect()
    }

    pub fn delete_card(&self, id: i64) -> Result<()> {
        let rows = self.conn.execute("DELETE FROM cards WHERE id = ?1", [id])?;
        if rows == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    }

    pub fn review_card(&self, id: i64, rating: Rating) -> Result<DbCard> {
        let mut stmt = self.conn.prepare(
            "SELECT title, prompt, body, tags,
             stability, difficulty, elapsed_days, scheduled_days,
             reps, lapses, state, last_review, created_at FROM cards WHERE id = ?1",
        )?;

        let (title, prompt, body, tags, card_state, created_at) =
            stmt.query_row([id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    CardState {
                        stability: row.get(4)?,
                        difficulty: row.get(5)?,
                        elapsed_days: row.get(6)?,
                        scheduled_days: row.get(7)?,
                        reps: row.get(8)?,
                        lapses: row.get(9)?,
                        state: int_to_state(row.get(10)?),
                        last_review: row.get(11)?,
                    },
                    row.get::<_, i64>(12)?,
                ))
            })?;

        let now = chrono::Utc::now().timestamp();
        let now_days = now as f64 / 86400.0;
        let (next_state, interval_secs) = card_state.review(rating, now_days);

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

        Ok(DbCard {
            id,
            title,
            prompt,
            body,
            tags,
            fsrs_state: next_state,
            due_at: now + interval_secs as i64,
            created_at,
        })
    }

    /// Filter cards by tag (case-insensitive substring match on the tags field).
    pub fn get_cards_by_tag(&self, tag: &str) -> Result<Vec<DbCard>> {
        // Use ESCAPE so % and _ in the tag are treated as literals, not wildcards.
        let mut stmt = self.conn.prepare(
            "SELECT id, title, prompt, body, tags,
             stability, difficulty, elapsed_days, scheduled_days,
             reps, lapses, state, last_review, due_at, created_at
             FROM cards WHERE tags LIKE ?1 ESCAPE '\\' ORDER BY created_at DESC",
        )?;

        let escaped = tag.to_lowercase()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let cards = stmt.query_map([pattern], |row| {
            Ok(DbCard {
                id: row.get(0)?,
                title: row.get(1)?,
                prompt: row.get(2)?,
                body: row.get(3)?,
                tags: row.get(4)?,
                fsrs_state: CardState {
                    stability: row.get(5)?,
                    difficulty: row.get(6)?,
                    elapsed_days: row.get(7)?,
                    scheduled_days: row.get(8)?,
                    reps: row.get(9)?,
                    lapses: row.get(10)?,
                    state: int_to_state(row.get(11)?),
                    last_review: row.get(12)?,
                },
                due_at: row.get(13)?,
                created_at: row.get(14)?,
            })
        })?;

        cards.collect()
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

    // ── Initialization ───────────────────────────────────────────────

    #[test]
    fn add_single_card() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db
            .add_card("Title", None, "Body", "")
            .expect("Failed to add card");
        assert_eq!(id, 1);
    }

    #[test]
    fn add_multiple_cards() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id1 = db.add_card("T1", None, "B1", "").expect("add 1");
        let id2 = db.add_card("T2", None, "B2", "").expect("add 2");
        let id3 = db.add_card("T3", None, "B3", "").expect("add 3");
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn added_card_has_new_state() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("Title", None, "Body", "").expect("add");
        let cards = db.get_all_cards().expect("get");
        assert_eq!(cards[0].fsrs_state.state, State::New);
    }

    #[test]
    fn added_card_has_default_fsrs_values() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("Title", None, "Body", "").expect("add");
        let card = &db.get_all_cards().expect("get")[0];
        assert_eq!(card.fsrs_state.stability, 0.0);
        assert_eq!(card.fsrs_state.difficulty, 0.0);
        assert_eq!(card.fsrs_state.reps, 0);
        assert_eq!(card.fsrs_state.lapses, 0);
    }

    // ── Content fields ───────────────────────────────────────────────

    #[test]
    fn card_with_prompt_stores_and_retrieves() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("comma tip", Some(", ffmpeg -i in out"), "Run any binary via nix-index", "nix,cli")
            .expect("add");
        let card = &db.get_all_cards().expect("get")[0];
        assert_eq!(card.title, "comma tip");
        assert_eq!(card.prompt, Some(", ffmpeg -i in out".to_string()));
        assert_eq!(card.body, "Run any binary via nix-index");
        assert_eq!(card.tags, "nix,cli");
    }

    #[test]
    fn card_without_prompt_is_null() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("HTTP 201?", None, "Resource created.", "http")
            .expect("add");
        let card = &db.get_all_cards().expect("get")[0];
        assert_eq!(card.prompt, None);
    }

    #[test]
    fn card_preserves_all_content_fields() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card(
            "Jump to end of file in vim",
            Some("G"),
            "Capital G. Use gg for top.",
            "vim,editor",
        )
        .expect("add");
        let card = &db.get_all_cards().expect("get")[0];
        assert_eq!(card.title, "Jump to end of file in vim");
        assert_eq!(card.prompt, Some("G".to_string()));
        assert_eq!(card.body, "Capital G. Use gg for top.");
        assert_eq!(card.tags, "vim,editor");
    }

    // ── Get All Cards ────────────────────────────────────────────────

    #[test]
    fn get_all_cards_empty() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let cards = db.get_all_cards().expect("get");
        assert_eq!(cards.len(), 0);
    }

    #[test]
    fn get_all_cards_retrieves_all() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("T1", None, "B1", "").expect("add 1");
        db.add_card("T2", None, "B2", "").expect("add 2");
        db.add_card("T3", None, "B3", "").expect("add 3");
        assert_eq!(db.get_all_cards().expect("get").len(), 3);
    }

    // ── Update Card ──────────────────────────────────────────────────

    #[test]
    fn update_card_changes_content() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db.add_card("Old title", None, "Old body", "").expect("add");
        db.update_card(id, "New title", Some("cmd"), "New body", "tag")
            .expect("update");
        let card = &db.get_all_cards().expect("get")[0];
        assert_eq!(card.title, "New title");
        assert_eq!(card.prompt, Some("cmd".to_string()));
        assert_eq!(card.body, "New body");
        assert_eq!(card.tags, "tag");
    }

    #[test]
    fn update_card_can_clear_prompt() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db
            .add_card("Title", Some("cmd"), "Body", "")
            .expect("add");
        db.update_card(id, "Title", None, "Body", "")
            .expect("update");
        let card = &db.get_all_cards().expect("get")[0];
        assert_eq!(card.prompt, None);
    }

    #[test]
    fn update_card_preserves_fsrs_state() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db.add_card("Title", None, "Body", "").expect("add");
        db.review_card(id, Rating::Good).expect("review");
        let state_before = db.get_all_cards().expect("get")[0].fsrs_state.state;
        db.update_card(id, "New title", None, "New body", "")
            .expect("update");
        let state_after = db.get_all_cards().expect("get")[0].fsrs_state.state;
        assert_eq!(state_before, state_after);
    }

    // ── Delete Card ──────────────────────────────────────────────────

    #[test]
    fn delete_card_removes_it() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db.add_card("Title", None, "Body", "").expect("add");
        db.delete_card(id).expect("delete");
        assert_eq!(db.get_all_cards().expect("get").len(), 0);
    }

    #[test]
    fn delete_specific_card_leaves_others() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id1 = db.add_card("T1", None, "B1", "").expect("add 1");
        let id2 = db.add_card("T2", None, "B2", "").expect("add 2");
        db.delete_card(id1).expect("delete");
        let cards = db.get_all_cards().expect("get");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, id2);
    }

    // ── Get Due Cards ────────────────────────────────────────────────

    #[test]
    fn newly_added_card_is_due() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("Title", None, "Body", "").expect("add");
        let now = chrono::Utc::now().timestamp();
        assert_eq!(db.get_due_cards(now).expect("get due").len(), 1);
    }

    #[test]
    fn reviewed_card_is_not_immediately_due() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db.add_card("Title", None, "Body", "").expect("add");
        db.review_card(id, Rating::Good).expect("review");
        let now = chrono::Utc::now().timestamp();
        assert_eq!(db.get_due_cards(now).expect("get due").len(), 0);
    }

    // ── Review Card ──────────────────────────────────────────────────

    #[test]
    fn review_increments_reps() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db.add_card("Title", None, "Body", "").expect("add");
        db.review_card(id, Rating::Good).expect("review");
        assert_eq!(db.get_all_cards().expect("get")[0].fsrs_state.reps, 1);
    }

    #[test]
    fn review_again_increments_lapses() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db.add_card("Title", None, "Body", "").expect("add");
        db.review_card(id, Rating::Good).expect("first review");
        db.review_card(id, Rating::Again).expect("lapse");
        assert_eq!(db.get_all_cards().expect("get")[0].fsrs_state.lapses, 1);
    }

    #[test]
    fn review_pushes_due_at_into_future() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db.add_card("Title", None, "Body", "").expect("add");
        let initial_due = db.get_all_cards().expect("get")[0].due_at;
        db.review_card(id, Rating::Good).expect("review");
        let final_due = db.get_all_cards().expect("get")[0].due_at;
        assert!(final_due > initial_due);
    }

    #[test]
    fn review_easy_gives_higher_stability_than_again() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id1 = db.add_card("T1", None, "B1", "").expect("add 1");
        let id2 = db.add_card("T2", None, "B2", "").expect("add 2");
        db.review_card(id1, Rating::Easy).expect("review easy");
        db.review_card(id2, Rating::Again).expect("review again");
        let cards = db.get_all_cards().expect("get");
        let s_easy = cards.iter().find(|c| c.id == id1).unwrap().fsrs_state.stability;
        let s_again = cards.iter().find(|c| c.id == id2).unwrap().fsrs_state.stability;
        assert!(s_easy > s_again);
    }

    #[test]
    fn review_returns_card_with_full_content() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        let id = db
            .add_card("comma tip", Some(", cmd"), "Details", "nix")
            .expect("add");
        let returned = db.review_card(id, Rating::Good).expect("review");
        assert_eq!(returned.title, "comma tip");
        assert_eq!(returned.prompt, Some(", cmd".to_string()));
        assert_eq!(returned.body, "Details");
        assert_eq!(returned.tags, "nix");
    }

    // ── Tag filtering ────────────────────────────────────────────────

    #[test]
    fn get_cards_by_tag_finds_match() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("T1", None, "B1", "nix,cli").expect("add 1");
        db.add_card("T2", None, "B2", "vim").expect("add 2");
        let results = db.get_cards_by_tag("nix").expect("tag query");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "T1");
    }

    #[test]
    fn get_cards_by_tag_is_case_insensitive() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("T1", None, "B1", "Nix,CLI").expect("add");
        let results = db.get_cards_by_tag("nix").expect("tag query");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn get_cards_by_tag_returns_empty_when_no_match() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("T1", None, "B1", "vim").expect("add");
        let results = db.get_cards_by_tag("nix").expect("tag query");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn get_cards_by_tag_matches_partial_tag_name() {
        let db = Database::open_in_memory().expect("Failed to create test db");
        db.add_card("T1", None, "B1", "nix,cli").expect("add 1");
        db.add_card("T2", None, "B2", "nixpkgs").expect("add 2");
        // both contain "nix"
        let results = db.get_cards_by_tag("nix").expect("tag query");
        assert_eq!(results.len(), 2);
    }

    // ── State conversion ─────────────────────────────────────────────

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
        for state in [State::New, State::Learning, State::Review, State::Relearning] {
            assert_eq!(int_to_state(state_to_int(state)), state);
        }
    }
}
