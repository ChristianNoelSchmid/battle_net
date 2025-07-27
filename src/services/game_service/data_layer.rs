use axum::async_trait;
use chrono::Utc;
use derive_more::Constructor;
use sqlx::SqlitePool;

use crate::{data_layer_error::Result, resources::game_resources::BaseStats};

use super::models::{CardModel, GameStateModel, Stats, UserCardModel};

const PERSON_CAT_IDX: i32 = 0;

#[async_trait]
pub trait GameDataLayer : Send + Sync {
    ///
    /// Checks if there is a game currently running
    /// 
    async fn is_game_active(&self) -> Result<bool>;
    async fn pl_guessed_today(&self, user_id: i64) -> Result<bool>;
    async fn update_guessed_today(&self, user_id: i64) -> Result<()>;
    ///
    /// Sets up the game with the given target cards (excluding the murdered user card,
    /// as that is generated in this process), and base user stats. 
    /// Returns the randomly chosen murdered user.
    ///
    async fn setup_game<'a>(&self, target_cards: &'a [CardModel], base_stats: &'a BaseStats) -> Result<()>;
    /// 
    /// Returns all current game state data, as it pertains to the particular user
    /// (ie. if the user has won, their collection of evidence cards, etc.)
    /// 
    async fn game_state(&self, user_id: i64) -> Result<Option<GameStateModel>>;
    ///
    /// Gets all target cards in the current game.
    /// The cards users have to guess to win.
    /// 
    async fn get_target_cards(&self) -> Result<Vec<CardModel>>;
    ///
    /// Assigns the user to the winner collection
    /// 
    async fn add_new_winner(&self, user_id: i64) -> Result<()>;
    ///
    /// Updates the status of a user's card, if required.
    /// Unconfirmed cards which are no longer guessed are removed, while
    /// Unconfirmed cards that are guessed are added to the user's data (if needed)
    /// 
    async fn update_user_card(&self, user_id: i64, cat_idx: i64, card_idx: i64, guessed: bool) -> Result<()>;
    ///
    /// Retrieves the indices of all evidence cards that the user has confirmed
    /// 
    async fn get_confirmed_user_cards(&self, user_id: i64) -> Result<Vec<CardModel>>;
    ///
    /// Adds card to user's confirmed set, marked as `confirmed`
    /// 
    async fn confirm_user_card(&self, user_id: i64, cat_idx: i64, card_idx: i64) -> Result<()>;
    async fn get_completed_riddle_count(&self, user_id: i64) -> Result<i64>;
}

#[derive(Constructor)]
pub struct DbGameDataLayer { 
    db: SqlitePool
}

#[async_trait]
impl GameDataLayer for DbGameDataLayer {
    async fn is_game_active(&self) -> Result<bool> {
        // Get the game state from the database (there should only be one)
        Ok(
            sqlx::query!("SELECT * FROM game_states")
            .fetch_optional(&self.db).await?.is_some()
        )
    }

    async fn pl_guessed_today(&self, user_id: i64) -> Result<bool> {
        Ok(
            sqlx::query!("SELECT guessed_today FROM users WHERE id = ?", user_id)
                .fetch_one(&self.db).await?.guessed_today
        )
    }

    async fn update_guessed_today(&self, user_id: i64) -> Result<()> {
        sqlx::query!("UPDATE users SET guessed_today = TRUE WHERE id = ?", user_id)
            .execute(&self.db).await?;
        Ok(())
    }

    async fn setup_game<'a>(&self, target_cards: &'a [CardModel], base_stats: &'a BaseStats) -> Result<()> {
        // Get all user ids and card_idxs
        let users = sqlx::query!("SELECT id, card_idx FROM users")
            .fetch_all(&self.db).await?;

        // Get all other user ids
        let user_ids: Vec<i64> = users.iter().map(|u| u.id).collect();

        // Insert base player stats for each user
        for id in user_ids {
            // Create the user's stats and add to the database
            let stats_id = sqlx::query!("
                INSERT INTO STATS (health, armor) VALUES (?, ?)
                ", base_stats.health, base_stats.armor
            ).execute(&self.db).await?.last_insert_rowid();

            // Associate the user to their stats
            sqlx::query!("
                INSERT INTO user_states (user_id, stats_id) VALUES (?, ?)
                ", id, stats_id,
            ).execute(&self.db).await?;
        }

        // Insert each generated target card into the game_target_cards table
        for target_card in target_cards {
            sqlx::query!("
                INSERT INTO game_target_cards (cat_idx, card_idx) VALUES (?, ?)
                ", target_card.cat_idx, target_card.card_idx
            ).execute(&self.db).await?;
        }

        // Add the initialized game state
        sqlx::query!("INSERT INTO game_states (murdered_user_id) VALUES (?)", 4)
            .execute(&self.db).await?;

        Ok(())
    }

    async fn game_state(&self, user_id: i64) -> Result<Option<GameStateModel>> {
        // Get the game state from the database (may not yet exist)
        let murdered_user_id = sqlx::query!("SELECT murdered_user_id FROM game_states")
            .fetch_optional(&self.db).await?
            .and_then(|row| Some(row.murdered_user_id));

        // If the game state does not exist, return None
        if murdered_user_id.is_none() {
            return Ok(None);
        }

        // Get the user's info
        let user = sqlx::query!("
            SELECT exhausted, riddle_quest_completed, guessed_today, last_login FROM users WHERE id = ?
            ", user_id
        ).fetch_one(&self.db).await?;
        
        // Determine if the given user has won the game
        let has_won = sqlx::query!("SELECT * FROM game_winners WHERE user_id = ?", user_id)
            .fetch_optional(&self.db).await?.is_some();

        // If the user has won, retrieve the target cards and all current winners
        let (mut target_cards, mut winner_idxs) = (None, None);

        if has_won {
            target_cards = Some(
                sqlx::query_as!(CardModel,
                    "SELECT cat_idx, card_idx FROM game_target_cards ORDER BY cat_idx ASC"
                ).fetch_all(&self.db).await?
            );
            
            winner_idxs = Some(
                sqlx::query!("
                    SELECT u.card_idx FROM game_winners gw 
                    JOIN users u ON gw.user_id = u.id 
                    ORDER BY gw.id ASC
                ")
                    .fetch_all(&self.db).await?
                    .iter().map(|row| row.card_idx).collect::<Vec<i64>>()
            );
        }

        // Get the user's current guessed cards and confirmed cards
        let user_cards = sqlx::query_as!(UserCardModel,
            "SELECT cat_idx, card_idx, confirmed FROM user_cards WHERE user_id = ?", 
            user_id
        ).fetch_all(&self.db).await?;

        let user_stats_id = sqlx::query!("SELECT stats_id FROM user_states WHERE user_id = ?", user_id)
            .fetch_one(&self.db).await?.stats_id;

        let user_stats = sqlx::query_as!(Stats,
            "SELECT power, health, armor, missing_next_turn FROM stats WHERE id = ?",
            user_stats_id
        )
            .fetch_one(&self.db).await?;

        // Update user to set last login to now
        let now = Utc::now();
        sqlx::query!("UPDATE users SET last_login = ? WHERE id = ?", now, user_id)
            .execute(&self.db).await?;

        Ok(Some(GameStateModel {
            user_id,
            target_cards, 
            user_cards, 
            user_stats,
            winner_idxs,
            murdered_user_id: murdered_user_id.unwrap(),
            pl_exhausted: user.exhausted,
            pl_completed_daily_riddle: user.riddle_quest_completed,
            pl_completed_all_riddles: false,
            pl_guessed_today: user.guessed_today,
            first_login: user.last_login.is_none() 
        }))
    }

    async fn get_target_cards(&self) -> Result<Vec<CardModel>> {
        Ok(
            sqlx::query_as!(CardModel, 
                "SELECT cat_idx, card_idx FROM game_target_cards"
            ).fetch_all(&self.db).await?
        )
    }

    async fn add_new_winner(&self, user_id: i64) -> Result<()> {
        sqlx::query!("INSERT INTO game_winners (user_id) VALUES (?)", user_id)
            .execute(&self.db).await?;
        Ok(())
    }

    async fn update_user_card(&self, user_id: i64, cat_idx: i64, card_idx: i64, guessed: bool) -> Result<()> {

        // Get the current value of the choice card
        let confirmed = sqlx::query!(
            "SELECT confirmed FROM user_cards WHERE user_id = ? AND cat_idx = ? AND card_idx = ?",
            user_id, cat_idx, card_idx
        ).fetch_optional(&self.db).await?.and_then(|row| Some(row.confirmed));

        match confirmed {
            // If there's no user card that matches and the user has guessed, insert
            // that card, unconfirmed
            None if guessed => {
                sqlx::query!(
                    "INSERT INTO user_cards (user_id, cat_idx, card_idx) VALUES (?, ?, ?)", 
                    user_id, cat_idx, card_idx
                ).execute(&self.db).await?;
            },
            // If a card does exist, is unconfirmed, and the user no longer guesses it,
            // delete that card
            Some(false) if !guessed => {
                sqlx::query!(
                    "DELETE FROM user_cards WHERE user_id = ? AND cat_idx = ? AND card_idx = ?",
                    user_id, cat_idx, card_idx
                ).execute(&self.db).await?;
            },
            // Ignore any other cases - they do not affect the user card
            // (card exists and is guessed doesn't matter, as there is already a card representing this)
            // (card confirmed but no longer guessed doesn't matter, as the card is confirmed)
            _ => {}
        }

        Ok(())
    }

    async fn get_confirmed_user_cards(&self, user_id: i64) -> Result<Vec<CardModel>> {
        Ok(
            sqlx::query_as!(CardModel,
                "SELECT cat_idx, card_idx FROM user_cards WHERE user_id = ? AND confirmed = TRUE",
                user_id
            )
                .fetch_all(&self.db).await?
        )
    }

    async fn confirm_user_card(&self, user_id: i64, cat_idx: i64, card_idx: i64) -> Result<()> {
        // If the card already exists in the users evidence cards, update it to confirmed
        let card = sqlx::query_as!(CardModel,
            "SELECT cat_idx, card_idx FROM user_cards 
            WHERE user_id = ? AND cat_idx = ? AND card_idx = ?",
            user_id, cat_idx, card_idx
        ).fetch_optional(&self.db).await?;

        if let Some(card) = card {
            sqlx::query!(
                "UPDATE user_cards SET confirmed = TRUE WHERE cat_idx = ? AND card_idx = ?",
                card.cat_idx, card.card_idx
            ).execute(&self.db).await?;
        // Otherwise, create a new user card as confirmed
        } else {
            sqlx::query!(
                "INSERT INTO user_cards (user_id, cat_idx, card_idx, confirmed) VALUES (?, ?, ?, TRUE)",
                user_id, cat_idx, card_idx
            ).execute(&self.db).await?;
        }

        Ok(())
    } 

    async fn get_completed_riddle_count(&self, user_id: i64) -> Result<i64> {
        Ok(
            sqlx::query!(
                "SELECT COUNT(*) AS count FROM quests WHERE quest_type = ? AND completed = TRUE AND user_id = ?",
                2, user_id
            ).fetch_one(&self.db).await?.count
        )
    }
}