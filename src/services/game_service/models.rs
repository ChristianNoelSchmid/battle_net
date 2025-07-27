use crate::resources::game_resources::BaseStats;
use derive_more::Constructor;
use serde::{Serialize, Deserialize};

#[derive(Debug, PartialEq, Serialize)]
pub struct CardModel {
    pub cat_idx: i64,
    pub card_idx: i64
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserCardModel {
    pub cat_idx: i64,
    pub card_idx: i64,
    pub confirmed: bool
}

///
/// Initial state of a game, directly after setup
/// 
#[derive(Serialize)]
pub struct GameInitialStateModel {
    pub target_cards: Vec<CardModel>,
}

#[derive(Serialize)]
pub struct GameStateModel {
    pub user_id: i64,
    pub murdered_user_id: i64,
    pub user_stats: Stats,
    pub user_cards: Vec<UserCardModel>,
    pub target_cards: Option<Vec<CardModel>>,
    pub winner_idxs: Option<Vec<i64>>,
    pub pl_exhausted: bool,
    pub pl_completed_daily_riddle: bool,
    pub pl_completed_all_riddles: bool,
    pub pl_guessed_today: bool,
    pub first_login: bool,
}

#[derive(Constructor, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Stats {
    pub health: i64,
    pub power: i64,
    pub armor: i64,
    pub missing_next_turn: bool,
}

#[derive(Debug, Serialize)]
pub enum GuessResult {
    #[serde(rename="correct")]
    Correct([i64;3]),
    #[serde(rename="incorrect")]
    Incorrect,
    #[serde(rename="already-won")]
    AlreadyWon,
    #[serde(rename="already-guessed-today")]
    AlreadyGuessedToday,
}

impl Stats {
    pub fn from_base_stats(b_stats: BaseStats) -> Self {
        Self::new(b_stats.health, 1, b_stats.armor, false)
    }
}