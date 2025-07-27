pub mod data_layer;
pub mod error;
pub mod models;
pub mod entities;

use std::sync::Arc;

use axum::async_trait;
use derive_more::Constructor;
use rand::{seq::{IteratorRandom, SliceRandom}, thread_rng, Rng};

use self::models::{
    QuestReward, RiddleStatus, QuestStateModel, QuestRiddleModel, QuestMonsterModel, QuestConsequences,
};

use crate::resources::game_resources::Resources;
use crate::data_layer_error::DataLayerError;

use self::{error::{Result, QuestServiceError}, data_layer::QuestDataLayer};

use super::game_service::{models::Stats, GameService};
use super::items_service::ItemsService;

#[async_trait]
pub trait QuestService: Send + Sync {
    ///
    /// Generates a new quest of the specified `quest_type` for the user with the given `user_id`.
    /// Throws Error if the user already has an active quest
    /// 
    async fn generate_quest(&self, user_id: i64, quest_type: i64) -> Result<QuestStateModel>;
    ///
    /// Returns the users current quest, if they are on one. Returns error if the user is not on a quest
    /// 
    async fn get_quest(&self, user_id: i64) -> Result<QuestStateModel>;
    ///
    /// Performs a guess on a riddle quest, which the user given the `user_id` has active.
    /// Throws Error if the user is not on a riddle quest
    /// 
    async fn guess_riddle(&self, user_id: i64, answer: String) -> Result<RiddleStatus>;
    ///
    /// Completes the quest the user with the given `user_id` is currently on.
    /// Returns a `QuestReward`, with new confirmed card for user (if not all cards are confirmed already)
    /// 
    async fn complete_quest(&self, user_id: i64) -> Result<QuestReward>;
    ///
    /// Completes te quest the user with the given `user_id` is currently on.
    /// Returns a `QuestConsequences`, which include ailments the user now has.  
    /// 
    async fn fail_quest(&self, user_id: i64) -> Result<QuestConsequences>;
}

#[derive(Constructor)]
pub struct CoreQuestService {
    data_layer: Arc<dyn QuestDataLayer>,
    res: Arc<Resources>,
    game_service: Arc<dyn GameService>,
    items_service: Arc<dyn ItemsService>,
}

#[async_trait]
impl QuestService for CoreQuestService {
    async fn generate_quest(&self, user_id: i64, quest_type: i64) -> Result<QuestStateModel> {
        let quest = self.data_layer.create_new_user_quest(user_id, quest_type).await.map_err(|e| e.into())?;
        let pl_lvl = self.data_layer.get_pl_lvl(user_id).await.map_err(|e| e.into())?;
        
        if let Some(quest) = quest {
            let (mut monster_state, mut riddle_state) = (None, None);
            match quest_type {
                0 => {
                    if self.data_layer.pl_is_exhausted(user_id).await.map_err(|e| e.into())? {
                        // If the player is exhausted, delete the quest that was just created
                        // and return the Error
                        self.data_layer.delete_quest(quest.id).await.map_err(|e| e.into())?;
                        return Err(QuestServiceError::PlayerIsExhausted);
                    }
                    monster_state = Some(self.generate_monster_quest(quest.id, pl_lvl).await?);
                }
                1 => {
                    match self.generate_riddle_quest(user_id, quest.id).await {
                        Ok(model) => riddle_state = Some(model),
                        Err(QuestServiceError::AllRiddlesCompleted) => {
                            // If all riddles were completed, delete the quest that was just created
                            // and return the Error
                            self.data_layer.delete_quest(quest.id).await.map_err(|e| e.into())?;
                            return Err(QuestServiceError::AllRiddlesCompleted);
                        },
                        Err(QuestServiceError::PlayerAlreadyCompletedRiddle) => {
                            // If player already completed riddle today, 
                            // delete the quest that was just created and return the Error
                            self.data_layer.delete_quest(quest.id).await.map_err(|e| e.into())?;
                            return Err(QuestServiceError::PlayerAlreadyCompletedRiddle);
                        },
                        Err(e) => return Err(e)
                    }
                }
                _ => { }
            }

            return Ok(QuestStateModel {
                quest_type: quest.quest_type,
                monster_state, riddle_state
            });
        } 

        Err(QuestServiceError::QuestAlreadyActive)
    }

    async fn get_quest(&self, user_id: i64) -> Result<QuestStateModel> {
        let quest = self.data_layer.get_active_user_quest(user_id).await.map_err(|e| e.into())?;
        match quest {
            None => return Err(QuestServiceError::UserNotOnQuest),
            Some(quest) => {
                let monster_state = quest.monster_state.and_then(
                    |ms| Some(QuestMonsterModel { stats: ms.stats, res_idx: ms.monster_idx })
                );
                let riddle_state = quest.riddle_idx.and_then(
                    |idx| {
                        let mut rng = thread_rng();
                        let ans_scramb = self.res.riddles[idx as usize].answer.clone();
                        let mut ans_scramb = ans_scramb.chars().collect::<Vec<char>>();
                        ans_scramb.shuffle(&mut rng);
                        let ans_scramb = ans_scramb.into_iter().collect();
                        println!("{ans_scramb}");

                        Some(QuestRiddleModel { 
                            text: self.res.riddles[idx as usize].text.clone(), 
                            ans_scramb
                        })
                    }
                );

                return Ok(QuestStateModel { quest_type: quest.quest_type, monster_state, riddle_state });
            }
        }
    }

    async fn guess_riddle(&self, user_id: i64, answer: String) -> Result<RiddleStatus> {
        // Lowercase answer for string-matching
        let answer = answer.to_lowercase();

        // Get the user's riddle quest index. Throw error if one isn't found
        // (ie. the user is not on a riddle quest)
        let riddle_idx = self.data_layer.get_quest_riddle_idx(user_id).await.map_err(|e| e.into())?
            .ok_or(QuestServiceError::UserNotOnRiddleQuest)?;

        let riddle = &self.res.riddles[riddle_idx as usize];

        // If the user provides any answer in the collection of answers for the riddle,
        // quest is successfully completed
        if riddle.answer.to_lowercase() == answer {
            return Ok(RiddleStatus::Correct(self.complete_quest(user_id).await.map_err(|e| e.into())?));
        }
        return Ok(RiddleStatus::Incorrect);
    }  

    ///
    /// Completes the quest the user with the given `user_id` is currently on.
    /// Returns a `QuestReward`, with new confirmed card for user (if not all cards are confirmed already)
    /// 
    async fn complete_quest(&self, user_id: i64) -> Result<QuestReward> {
        // Complete the quest
        self.data_layer.complete_quest(user_id).await.map_err(|e| e.into())?;

        if self.data_layer.pl_has_won_game(user_id).await.map_err(|e| e.into())? {
            return Ok(
                QuestReward {
                    item_idxs: vec![],
                    card: None
                },
            );
        }

        // Generate random item rewards (1-2 items)
        let mut rng = thread_rng();
        let num_items = if rng.gen_bool(0.3) { 2 } else { 1 }; // 30% chance for 2 items
        let mut awarded_item_idxs = Vec::new();

        for _ in 0..num_items {
            // Random item index from available items
            let item_idx = (0..self.res.items.len()).choose(&mut rng).unwrap();
            
            // Add the item to user's inventory
            let item_id = self.items_service.add_item_to_user(user_id as i32, item_idx as i32).await
                .map_err(|_| QuestServiceError::DataLayerError(crate::data_layer_error::DataLayerError::DatabaseError))?;

            awarded_item_idxs.push(item_idx as i64);
            log::info!("Awarded item {} (ID: {}) to user {}", item_idx, item_id, user_id);
        }

        // Get a new confirmed card
        let new_card = self.data_layer.get_rand_unconfirmed_card(user_id, &self.res.evd_cats_and_cards).await.map_err(|e| e.into())?;

        // Confirm it with the game service
        if let Some(card) = &new_card {
            self.game_service.confirm_user_card(user_id, card.cat_idx, card.card_idx).await
                .map_err(|e| e.into())?;
        }

        // Return the successful quest reward
        return Ok(
            QuestReward {
                item_idxs: awarded_item_idxs,
                card: new_card
            },
        );
    }

    async fn fail_quest(&self, user_id: i64) -> Result<QuestConsequences> {
        // Complete the quest
        self.data_layer.complete_quest(user_id).await.map_err(|e| e.into())?;
        self.data_layer.exhaust_pl(user_id).await.map_err(|e| e.into())?;
        Ok(QuestConsequences { sab_idxs: vec![] })
    }
}

impl CoreQuestService {
    async fn generate_monster_quest(&self, quest_id: i64, quest_level: i64) -> Result<QuestMonsterModel> {
        // Choose a new monster to fight the player
        let (monster_idx, monster) = self.res.monsters
            .iter().enumerate().filter(|(_, monster)| monster.level == quest_level)
            .choose(&mut thread_rng()).unwrap();

        let monster_idx = monster_idx as i64;

        self.data_layer.create_quest_monster(quest_id, monster_idx, monster.stats).await.map_err(|e| e.into())?;

        Ok(QuestMonsterModel {
            res_idx: monster_idx,
            stats: Stats::from_base_stats(monster.stats)
        })
    }

    pub async fn generate_riddle_quest(&self, user_id: i64, quest_id: i64) -> Result<QuestRiddleModel> {
        // Ensure the user hasn't already completed a riddle today
        if self.data_layer.pl_answered_riddle(user_id).await.map_err(|e| e.into())? {
            return Err(QuestServiceError::PlayerAlreadyCompletedRiddle)
        }

        let ans_riddle_idxs = self.data_layer.get_user_answered_riddle(user_id).await.map_err(|e| e.into())?;

        // Choose a new riddle to give the player,
        // that the player has not seen
        let idx_and_riddle = self.res.riddles
            .iter().enumerate()
            .filter(|(idx, _)| !ans_riddle_idxs.contains(&(*idx as i64)))
            .choose(&mut thread_rng());

        return if let Some((idx, riddle)) = idx_and_riddle {
            self.data_layer.create_quest_riddle(quest_id, idx as i64).await.map_err(|e| e.into())?;

            Ok({
                let mut rng = thread_rng();
                let ans_scramb = riddle.answer.clone();
                let mut ans_scramb = ans_scramb.chars().collect::<Vec<char>>();
                ans_scramb.shuffle(&mut rng);

                QuestRiddleModel { 
                    text: riddle.text.clone(), 
                    ans_scramb: ans_scramb.into_iter().collect()
                }
            })
        } else {
            Err(QuestServiceError::AllRiddlesCompleted)
        };
    } 
}