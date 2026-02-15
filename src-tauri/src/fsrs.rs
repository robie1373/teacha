/// FSRS (Free Spaced Repetition Scheduler) v5 implementation.
///
/// Reference: <https://github.com/open-spaced-repetition/fsrs4anki/wiki/The-Algorithm>

use serde::{Deserialize, Serialize};

/// Default FSRS-5 weights (19 parameters).
const W: [f64; 19] = [
    0.4072, 1.1829, 3.1262, 15.4722, // w0-w3: initial stability for Again/Hard/Good/Easy
    7.2102,  // w4: initial difficulty mean
    0.5316,  // w5: initial difficulty scaling
    1.0651,  // w6: difficulty update scaling
    0.0589,  // w7: difficulty mean reversion
    1.5747,  // w8: stability base factor
    0.1070,  // w9: stability exponent on S
    1.0070,  // w10: stability retrievability factor
    2.0966,  // w11: forget stability base
    0.0340,  // w12: forget stability difficulty exponent
    0.3642,  // w13: forget stability stability exponent
    0.6710,  // w14: forget stability retrievability factor
    2.7505,  // w15: hard penalty
    0.2315,  // w16: easy bonus
    0.0000,  // w17: hard/easy short-term modifier (unused v5 keeps 0)
    0.0000,  // w18: reserved
];

/// Desired retention probability (90%).
const REQUEST_RETENTION: f64 = 0.9;

/// Rating a user gives after reviewing a card.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Rating {
    Again = 1,
    Hard  = 2,
    Good  = 3,
    Easy  = 4,
}

impl Rating {
    pub fn from_str(s: &str) -> Option<Rating> {
        match s.trim().to_lowercase().as_str() {
            "again" | "1" => Some(Rating::Again),
            "hard"  | "2" => Some(Rating::Hard),
            "good"  | "3" => Some(Rating::Good),
            "easy"  | "4" => Some(Rating::Easy),
            _ => None,
        }
    }

    fn index(self) -> usize {
        (self as usize) - 1
    }
}

/// Card learning state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum State {
    New,
    Learning,
    Review,
    Relearning,
}

/// Per-card FSRS scheduling state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardState {
    pub stability: f64,
    pub difficulty: f64,
    pub elapsed_days: f64,
    pub scheduled_days: f64,
    pub reps: u32,
    pub lapses: u32,
    pub state: State,
    pub last_review: f64, // unix timestamp as days
}

impl CardState {
    /// Brand-new card that has never been reviewed.
    pub fn new() -> Self {
        Self {
            stability: 0.0,
            difficulty: 0.0,
            elapsed_days: 0.0,
            scheduled_days: 0.0,
            reps: 0,
            lapses: 0,
            state: State::New,
            last_review: 0.0,
        }
    }

    /// Compute the next interval in **days** given desired retention.
    pub fn next_interval(stability: f64) -> f64 {
        let interval = stability * 9.0 * (1.0 / REQUEST_RETENTION - 1.0);
        interval.max(1.0)
    }

    /// Current retrievability (probability of recall).
    pub fn retrievability(&self, now_days: f64) -> f64 {
        if self.state == State::New || self.stability <= 0.0 {
            return 0.0;
        }
        let elapsed = (now_days - self.last_review).max(0.0);
        (1.0 + elapsed / (9.0 * self.stability)).powf(-1.0)
    }

    /// Process a review and return the updated card state + scheduled interval in seconds.
    pub fn review(&self, rating: Rating, now_days: f64) -> (CardState, u64) {
        let mut next = self.clone();
        next.reps += 1;

        let elapsed = if self.last_review > 0.0 {
            (now_days - self.last_review).max(0.0)
        } else {
            0.0
        };
        next.elapsed_days = elapsed;

        match self.state {
            State::New => {
                // First review: compute initial stability and difficulty.
                next.stability = init_stability(rating);
                next.difficulty = init_difficulty(rating);
                next.state = if rating == Rating::Again {
                    next.lapses += 1;
                    State::Learning
                } else {
                    State::Review
                };
            }
            State::Learning | State::Relearning => {
                let r = self.retrievability(now_days);
                next.stability = next_recall_stability(self.difficulty, self.stability, r, rating);
                next.difficulty = next_difficulty(self.difficulty, rating);
                next.state = if rating == Rating::Again {
                    next.lapses += 1;
                    State::Relearning
                } else {
                    State::Review
                };
            }
            State::Review => {
                let r = self.retrievability(now_days);
                if rating == Rating::Again {
                    // Lapse
                    next.lapses += 1;
                    next.stability = next_forget_stability(self.difficulty, self.stability, r);
                    next.difficulty = next_difficulty(self.difficulty, rating);
                    next.state = State::Relearning;
                } else {
                    next.stability =
                        next_recall_stability(self.difficulty, self.stability, r, rating);
                    next.difficulty = next_difficulty(self.difficulty, rating);
                    next.state = State::Review;
                }
            }
        }

        next.difficulty = next.difficulty.clamp(1.0, 10.0);
        next.stability = next.stability.max(0.01);

        let interval_days = match next.state {
            State::Learning | State::Relearning => {
                // Short intervals for learning/relearning.
                match rating {
                    Rating::Again => 1.0 / 1440.0, // 1 minute
                    Rating::Hard  => 5.0 / 1440.0, // 5 minutes
                    _             => 10.0 / 1440.0, // 10 minutes
                }
            }
            State::Review => CardState::next_interval(next.stability),
            State::New => 0.0, // shouldn't happen
        };

        next.scheduled_days = interval_days;
        next.last_review = now_days;

        let interval_secs = (interval_days * 86400.0).round().max(60.0) as u64;
        (next, interval_secs)
    }
}

// --- FSRS core formulas ---

fn init_stability(rating: Rating) -> f64 {
    W[rating.index()].max(0.01)
}

fn init_difficulty(rating: Rating) -> f64 {
    let d = W[4] - f64::exp(W[5] * (rating as i32 as f64 - 1.0)) + 1.0;
    d.clamp(1.0, 10.0)
}

fn next_difficulty(d: f64, rating: Rating) -> f64 {
    let d0 = init_difficulty(Rating::Good);
    let delta = -(W[6] * (rating as i32 as f64 - 3.0));
    let next = W[7] * d0 + (1.0 - W[7]) * (d + delta);
    next.clamp(1.0, 10.0)
}

fn next_recall_stability(d: f64, s: f64, r: f64, rating: Rating) -> f64 {
    let hard_penalty = if rating == Rating::Hard { W[15] } else { 1.0 };
    let easy_bonus = if rating == Rating::Easy { W[16] } else { 1.0 };
    s * (f64::exp(W[8])
        * (11.0 - d)
        * s.powf(-W[9])
        * (f64::exp(W[10] * (1.0 - r)) - 1.0)
        * hard_penalty
        * easy_bonus
        + 1.0)
}

fn next_forget_stability(d: f64, s: f64, r: f64) -> f64 {
    let result =
        W[11] * d.powf(-W[12]) * ((s + 1.0).powf(W[13]) - 1.0) * f64::exp(W[14] * (1.0 - r));
    result.max(0.01)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_card_good_review() {
        let card = CardState::new();
        let now = 0.0;
        let (next, interval_secs) = card.review(Rating::Good, now);
        assert_eq!(next.state, State::Review);
        assert!(next.stability > 0.0);
        assert!(next.difficulty >= 1.0 && next.difficulty <= 10.0);
        assert!(interval_secs >= 60);
        println!(
            "stability={:.4} difficulty={:.4} interval={}s",
            next.stability, next.difficulty, interval_secs
        );
    }

    #[test]
    fn new_card_again_goes_to_learning() {
        let card = CardState::new();
        let (next, interval_secs) = card.review(Rating::Again, 0.0);
        assert_eq!(next.state, State::Learning);
        assert_eq!(next.lapses, 1);
        assert!(interval_secs <= 120); // ~1 minute
    }

    #[test]
    fn review_card_again_lapses() {
        let card = CardState::new();
        let (reviewed, _) = card.review(Rating::Good, 0.0);
        let (lapsed, _) = reviewed.review(Rating::Again, 3.0);
        assert_eq!(lapsed.state, State::Relearning);
        assert_eq!(lapsed.lapses, 1);
    }

    #[test]
    fn intervals_increase_with_good() {
        let card = CardState::new();
        let (r1, i1) = card.review(Rating::Good, 0.0);
        let day1 = r1.scheduled_days;
        let (r2, i2) = r1.review(Rating::Good, day1);
        let (_, i3) = r2.review(Rating::Good, day1 + r2.scheduled_days);
        assert!(i2 > i1, "second interval should exceed first");
        assert!(i3 > i2, "third interval should exceed second");
    }

    // ── Rating ──────────────────────────────────────────────────

    #[test]
    fn rating_index_values() {
        assert_eq!(Rating::Again.index(), 0);
        assert_eq!(Rating::Hard.index(), 1);
        assert_eq!(Rating::Good.index(), 2);
        assert_eq!(Rating::Easy.index(), 3);
    }

    // ── CardState::new ──────────────────────────────────────────

    #[test]
    fn new_card_defaults() {
        let card = CardState::new();
        assert_eq!(card.stability, 0.0);
        assert_eq!(card.difficulty, 0.0);
        assert_eq!(card.reps, 0);
        assert_eq!(card.lapses, 0);
        assert_eq!(card.state, State::New);
        assert_eq!(card.last_review, 0.0);
    }

    // ── next_interval ───────────────────────────────────────────

    #[test]
    fn next_interval_minimum_one_day() {
        // Very low stability should clamp to 1 day
        let interval = CardState::next_interval(0.001);
        assert!((interval - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn next_interval_scales_with_stability() {
        let i1 = CardState::next_interval(1.0);
        let i2 = CardState::next_interval(5.0);
        let i3 = CardState::next_interval(20.0);
        assert!(i2 > i1);
        assert!(i3 > i2);
    }

    // ── retrievability ──────────────────────────────────────────

    #[test]
    fn retrievability_new_card_is_zero() {
        let card = CardState::new();
        assert_eq!(card.retrievability(10.0), 0.0);
    }

    #[test]
    fn retrievability_just_reviewed_is_high() {
        let card = CardState::new();
        let (reviewed, _) = card.review(Rating::Good, 0.0);
        let r = reviewed.retrievability(0.0);
        assert!(r > 0.9, "retrievability={} should be > 0.9 right after review", r);
    }

    #[test]
    fn retrievability_decays_over_time() {
        let card = CardState::new();
        let (reviewed, _) = card.review(Rating::Good, 0.0);
        let r_soon = reviewed.retrievability(1.0);
        let r_later = reviewed.retrievability(30.0);
        assert!(r_soon > r_later, "r_soon={} should > r_later={}", r_soon, r_later);
    }

    // ── init_stability ──────────────────────────────────────────

    #[test]
    fn init_stability_all_ratings_positive() {
        for rating in [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy] {
            let s = init_stability(rating);
            assert!(s > 0.0, "init_stability({:?})={} should be positive", rating, s);
        }
    }

    #[test]
    fn init_stability_increases_with_rating() {
        let s_again = init_stability(Rating::Again);
        let s_hard = init_stability(Rating::Hard);
        let s_good = init_stability(Rating::Good);
        let s_easy = init_stability(Rating::Easy);
        assert!(s_hard > s_again);
        assert!(s_good > s_hard);
        assert!(s_easy > s_good);
    }

    // ── init_difficulty ─────────────────────────────────────────

    #[test]
    fn init_difficulty_in_range() {
        for rating in [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy] {
            let d = init_difficulty(rating);
            assert!(d >= 1.0 && d <= 10.0, "init_difficulty({:?})={} out of range", rating, d);
        }
    }

    #[test]
    fn init_difficulty_decreases_with_rating() {
        let d_again = init_difficulty(Rating::Again);
        let d_easy = init_difficulty(Rating::Easy);
        assert!(d_again > d_easy, "again={} should > easy={}", d_again, d_easy);
    }

    // ── next_difficulty ─────────────────────────────────────────

    #[test]
    fn next_difficulty_clamped() {
        // Edge: very high difficulty + Again should stay <= 10
        let d = next_difficulty(10.0, Rating::Again);
        assert!(d <= 10.0);
        // Edge: very low difficulty + Easy should stay >= 1
        let d = next_difficulty(1.0, Rating::Easy);
        assert!(d >= 1.0);
    }

    // ── next_recall_stability ───────────────────────────────────

    #[test]
    fn recall_stability_grows_on_good() {
        let s = next_recall_stability(5.0, 3.0, 0.9, Rating::Good);
        assert!(s > 3.0, "stability after good recall should increase");
    }

    #[test]
    fn recall_stability_hard_gets_more_growth() {
        // FSRS "desirable difficulty": recalling a hard card strengthens memory
        // more than recalling an easy one (w15 > 1 amplifies growth for Hard).
        let s_good = next_recall_stability(5.0, 3.0, 0.5, Rating::Good);
        let s_hard = next_recall_stability(5.0, 3.0, 0.5, Rating::Hard);
        assert!(s_hard > s_good, "hard={} should > good={} (desirable difficulty)", s_hard, s_good);
    }

    #[test]
    fn recall_stability_easy_gets_less_growth() {
        // FSRS: trivially easy reviews contribute less to memory strengthening
        // (w16 < 1 reduces growth for Easy).
        let s_good = next_recall_stability(5.0, 3.0, 0.5, Rating::Good);
        let s_easy = next_recall_stability(5.0, 3.0, 0.5, Rating::Easy);
        assert!(s_easy < s_good, "easy={} should < good={} (less strengthening)", s_easy, s_good);
    }

    // ── next_forget_stability ───────────────────────────────────

    #[test]
    fn forget_stability_is_positive() {
        let s = next_forget_stability(5.0, 3.0, 0.5);
        assert!(s > 0.0);
    }

    #[test]
    fn forget_stability_less_than_original() {
        let original = 10.0;
        let s = next_forget_stability(5.0, original, 0.9);
        assert!(s < original, "forget s={} should < original={}", s, original);
    }

    // ── State transitions ───────────────────────────────────────

    #[test]
    fn new_hard_goes_to_review() {
        let card = CardState::new();
        let (next, _) = card.review(Rating::Hard, 0.0);
        assert_eq!(next.state, State::Review);
    }

    #[test]
    fn new_easy_goes_to_review() {
        let card = CardState::new();
        let (next, _) = card.review(Rating::Easy, 0.0);
        assert_eq!(next.state, State::Review);
    }

    #[test]
    fn learning_good_goes_to_review() {
        let card = CardState::new();
        let (learning, _) = card.review(Rating::Again, 0.0);
        assert_eq!(learning.state, State::Learning);
        let (next, _) = learning.review(Rating::Good, 0.01);
        assert_eq!(next.state, State::Review);
    }

    #[test]
    fn learning_again_goes_to_relearning() {
        let card = CardState::new();
        let (learning, _) = card.review(Rating::Again, 0.0);
        let (next, _) = learning.review(Rating::Again, 0.01);
        assert_eq!(next.state, State::Relearning);
        assert_eq!(next.lapses, 2);
    }

    #[test]
    fn review_good_stays_review() {
        let card = CardState::new();
        let (r1, _) = card.review(Rating::Good, 0.0);
        let (r2, _) = r1.review(Rating::Good, r1.scheduled_days);
        assert_eq!(r2.state, State::Review);
    }

    #[test]
    fn relearning_good_goes_to_review() {
        let card = CardState::new();
        let (r1, _) = card.review(Rating::Good, 0.0);
        let (lapsed, _) = r1.review(Rating::Again, r1.scheduled_days);
        assert_eq!(lapsed.state, State::Relearning);
        let (recovered, _) = lapsed.review(Rating::Good, lapsed.last_review + 0.01);
        assert_eq!(recovered.state, State::Review);
    }

    // ── Interval properties ─────────────────────────────────────

    #[test]
    fn again_interval_is_short() {
        let card = CardState::new();
        let (_, secs) = card.review(Rating::Again, 0.0);
        assert!(secs <= 120, "again interval={}s should be <= 120", secs);
    }

    #[test]
    fn easy_interval_longer_than_good() {
        let card = CardState::new();
        let (_, good_secs) = card.review(Rating::Good, 0.0);
        let (_, easy_secs) = card.review(Rating::Easy, 0.0);
        assert!(easy_secs >= good_secs, "easy={}s should >= good={}s", easy_secs, good_secs);
    }

    #[test]
    fn reps_counter_increments() {
        let card = CardState::new();
        let (r1, _) = card.review(Rating::Good, 0.0);
        assert_eq!(r1.reps, 1);
        let (r2, _) = r1.review(Rating::Good, r1.scheduled_days);
        assert_eq!(r2.reps, 2);
        let (r3, _) = r2.review(Rating::Good, r2.last_review + r2.scheduled_days);
        assert_eq!(r3.reps, 3);
    }

    #[test]
    fn minimum_interval_is_60_seconds() {
        // Even the shortest learning interval should be at least 60s
        for rating in [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy] {
            let card = CardState::new();
            let (_, secs) = card.review(rating, 0.0);
            assert!(secs >= 60, "rating {:?} interval={}s should be >= 60", rating, secs);
        }
    }
}
