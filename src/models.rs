use serde::{Deserialize, Serialize};
use sqlite::Row;

pub trait Model {
    fn from_row(row: Row) -> Self;
}

#[derive(Serialize, Clone)]
pub struct User {
    pub id: i64,
    pub card_id: i64,
    pub user_name: String,
    pub user_img_path: Option<String>,
}
impl Model for User {
    fn from_row(row: Row) -> Self {
        User {
            id: row.get("id"),
            card_id: row.get("card_id"),
            user_name: row.get("user_name"),
            user_img_path: row.try_get("user_img_path").ok(),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct EvidenceCard {
    pub card_id: i64,
    pub item_name: String,
    pub item_img_path: Option<String>,
}

impl Model for EvidenceCard {
    fn from_row(row: Row) -> Self {
        EvidenceCard {
            card_id: row.get("id"),
            item_name: row.get("item_name"),
            item_img_path: row.get("item_img_path")
        }
    }
}

#[derive(Serialize, Clone)]
pub struct UserState {
    pub confirmed_card_ids: Vec<i64>,
    pub unconfirmed_card_ids: Vec<i64>,
    pub current_riddle: Option<(i64, String)>,
}

#[derive(Serialize)]
pub struct GameInitialState {
    pub target_cards: Vec<(i64, String)>,
    pub murdered_user: (i64, String),
}

#[derive(Serialize)]
pub struct GameState {
    pub game_state_id: i64,
    pub murdered_user: User,
    pub categories: Vec<(i64, String)>,
    pub cards: Vec<EvidenceCard>,
    pub target_cards: Option<Vec<EvidenceCard>>,
    pub winners: Option<Vec<User>>,
}

#[derive(Serialize, Clone)]
pub struct Riddle {
    pub id: i64,
    pub text: String,
}

#[derive(Serialize)]
pub enum RiddleProgress {
    Correct((Option<Riddle>, Option<EvidenceCard>)),
    Incorrect,
}

#[derive(Deserialize)]
pub struct PostRiddle {
    pub text: String,
    pub answers: Vec<String>,
}
