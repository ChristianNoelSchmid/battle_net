use std::sync::Arc;

use axum::async_trait;
use derive_more::Constructor;
use rand::{thread_rng, RngCore};

use crate::ai::AI;
use crate::resources::game_resources::{Resources, Monster, ItemType};

use self::data_layer::{BattleDataLayer, ATTACK_IDX};
use self::error::{Result, BattleServiceError};
use self::models::{RoundResult, NextAction};

use super::game_service::models::Stats;
use super::quest_service::QuestService;
use super::items_service::ItemsService;

pub mod data_layer;
pub mod error;
pub mod models;

const PL_DMG: [(i64, i64);4] = [(1, 2), (3, 6), (5, 10), (8, 14)];
const MAX_POWER: i64 = 4;

#[async_trait]
pub trait BattleService : Send + Sync {
    ///
    /// Initializes the battle, or sends the current battle to the player
    /// on initial connection.
    /// 
    async fn setup(&self, user_id: i64) -> Result<RoundResult>;
    async fn attack(&self, user_id: i64, power: i64) -> Result<RoundResult>;
    async fn defend(&self, user_id: i64) -> Result<RoundResult>;
    async fn use_item(&self, user_id: i64, item_id: i64) -> Result<RoundResult>;
}

#[derive(Constructor)]
pub struct CoreBattleService {
    data_layer: Arc<dyn BattleDataLayer>,
    quest_service: Arc<dyn QuestService>,
    items_service: Arc<dyn ItemsService>,
    res: Arc<Resources>,
}

#[async_trait]
impl BattleService for CoreBattleService {
    async fn setup(&self, user_id: i64) -> Result<RoundResult> {
        let (mut pl_stats, mut monst_stats) = self.data_layer.get_pl_and_monst_stats(user_id).await.map_err(|e| e.into())?;
        let monster_state = self.data_layer.get_monst_state(user_id).await.map_err(|e| e.into())?;
        let next_action;

        if monster_state.next_action.is_none() {
            let monst_state = self.data_layer.get_monst_state(user_id).await.map_err(|e| e.into())?;
            let monst_res = &self.res.monsters[monst_state.res_idx];

            next_action = self.get_monster_next_action(monst_res, &pl_stats, &monst_stats);
            self.data_layer.set_monst_next_action(monst_state.db_id, &next_action).await.map_err(|e| e.into())?;

            self.data_layer.expend_pl_pow(user_id).await.map_err(|e| e.into())?;
            self.data_layer.increment_pl_pow(user_id, MAX_POWER).await.map_err(|e| e.into())?;

            // Re-retrieve the player and monster stats with them set up
            (pl_stats, monst_stats) = self.data_layer.get_pl_and_monst_stats(user_id).await.map_err(|e| e.into())?;
        } else {
            next_action = monster_state.next_action.unwrap();
        }
        
        Ok(RoundResult::Next { pl_stats, monst_stats, next_action, pl_dmg_dealt: 0, monst_dmg_dealt: 0, monst_pow_used: 0 })
    }

    async fn attack(&self, user_id: i64, power: i64) -> Result<RoundResult> { 
        let pl_power = self.data_layer.get_pl_power(user_id).await.map_err(|e| e.into())?;
        // Check that the player has enough power
        if pl_power < power {
            return Err(BattleServiceError::NotEnoughPower);
        }
        if power < 1 || power > MAX_POWER {
            return Err(BattleServiceError::PowerOutOfRange);
        }

        let dmg_rng = PL_DMG[(power -  1) as usize];
        let dmg = (thread_rng().next_u32() as i64) % (dmg_rng.1 - dmg_rng.0);
        let dmg = dmg_rng.0 + dmg;

        let (dmg, defeated) = self.data_layer.dmg_monst(user_id, power, dmg).await.map_err(|e| e.into())?;

        // Damage the monster, and test if it's been defeated
        return if defeated {
            // If it was defeated, complete the quest and return the victory signal, with rewards
            let reward = self.quest_service.complete_quest(user_id).await.map_err(|e| BattleServiceError::QuestServiceError(e))?;
            Ok(RoundResult::Victory { reward, pl_dmg_dealt: dmg })
        } else {
            // Otherwise, perform the monster's action, and return the results
            self.perform_monster_action(user_id, false, dmg).await.map_err(|e| e.into())
        }
    }
    async fn defend(&self, user_id: i64) -> Result<RoundResult> {
        self.perform_monster_action(user_id, true, 0).await.map_err(|e| e.into())
    }
    async fn use_item(&self, user_id: i64, item_id: i64) -> Result<RoundResult> { 
        // Use the item through the items service
        let item_result = self.items_service.use_item(user_id as i32, item_id as i32).await
            .map_err(|e| BattleServiceError::ItemsServiceError(format!("{:?}", e)))?;

        if !item_result.success {
            return Err(BattleServiceError::ItemsServiceError(item_result.message));
        }

        // If the item has weapon damage, apply it to the monster
        let dmg_dealt = if let Some(weapon_damage) = self.get_weapon_damage_from_result(&item_result) {
            let (dmg, defeated) = self.data_layer.dmg_monst(user_id, 1, weapon_damage).await.map_err(|e| e.into())?;
            if defeated {
                let reward = self.quest_service.complete_quest(user_id).await.map_err(|e| e.into())?;
                return Ok(RoundResult::Victory { reward, pl_dmg_dealt: dmg });
            }
            dmg
        } else {
            0
        };

        // Perform monster action if not defeated
        self.perform_monster_action(user_id, false, dmg_dealt).await.map_err(|e| e.into())
    }
}

impl CoreBattleService {
    pub fn get_action_flv_txt<'a>(&self, monst_stats: &Stats, monst_res: &'a Monster, action: i64) -> &'a str {
        return match action {
            data_layer::ATTACK_IDX => monst_res.attack_flv_texts[(monst_stats.power - 1) as usize].as_str(),
            data_layer::DEFEND_IDX => monst_res.defend_flv_texts[(monst_stats.power - 1) as usize].as_str(),
            data_layer::IDLE_IDX => monst_res.idle_flv_texts[(monst_stats.power - 1) as usize].as_str(),
            _ => "Unknown action"
        };
    }

    fn get_weapon_damage_from_result(&self, item_result: &super::items_service::ItemUseResult) -> Option<i64> {
        // Check if the message contains weapon damage information
        if item_result.message.contains("damage") {
            // Try to extract damage from message like "Attacking with Iron Sword! +5 damage"
            if let Some(damage_part) = item_result.message.split('+').nth(1) {
                if let Some(damage_str) = damage_part.split(' ').next() {
                    return damage_str.parse::<i64>().ok();
                }
            }
        }
        None
    }

    ///
    /// Returns the amount of damage the monster does this turn, given the info
    /// 
    fn get_monster_dmg(&self, monst_stats: &Stats, monst_res: &Monster, pl_defd: bool) -> i64 {
        let rng = monst_res.pow_dmg[(monst_stats.power - 1) as usize];
        let dmg = (thread_rng().next_u32() % (rng.1 - rng.0) as u32) as i64;

        return if !pl_defd { rng.0 + dmg } else { ((rng.0 + dmg) as f32 / 2.0) as i64 };
    }
    
    ///
    /// Performs the monster's action, damaging the player if attacking,
    /// and generating its next action
    /// 
    async fn perform_monster_action(&self, user_id: i64, pl_defd: bool, pl_dmg_dealt: i64) -> Result<RoundResult> {
        // Get the current state of the Monster, and current player and Monster Stats
        let (pl_stats, monst_stats) = self.data_layer.get_pl_and_monst_stats(user_id).await.map_err(|e| e.into())?;
        let monst_state = self.data_layer.get_monst_state(user_id).await.map_err(|e| e.into())?;
        let monst_res = &self.res.monsters[monst_state.res_idx];
        let mut monst_dmg_dealt = 0i64;
        let mut monst_pow_used = 0i64;

        // Have the monster attack the player, and determine if the player is defeated
        if monst_state.next_action.unwrap().idx == ATTACK_IDX {
            monst_dmg_dealt = self.get_monster_dmg(&monst_stats, &self.res.monsters[monst_state.res_idx], pl_defd);
            monst_pow_used = monst_stats.power;
            if monst_dmg_dealt >= pl_stats.health {
                let consq = self.quest_service.fail_quest(user_id).await.map_err(|e| BattleServiceError::QuestServiceError(e))?;
                return Ok(RoundResult::Defeat { monst_dmg: monst_dmg_dealt, consq, pl_dmg_dealt, monst_pow_used: monst_stats.power })
            }
            self.data_layer.dmg_pl(user_id, monst_dmg_dealt).await.map_err(|e| e.into())?;
            self.data_layer.expend_monst_pow(monst_state.db_id).await.map_err(|e| e.into())?;
        }
        // Increment the player and monster's power by 1
        self.data_layer.increment_pl_pow(user_id, MAX_POWER).await.map_err(|e| e.into())?;
        self.data_layer.increment_monst_pow(monst_state.db_id, monst_res.pow_dmg.len() as i64).await.map_err(|e| e.into())?;

        // Determine the monster's next action, and new Stats
        let (pl_stats, monst_stats) = self.data_layer.get_pl_and_monst_stats(user_id).await.map_err(|e| e.into())?;
        let next_action = self.get_monster_next_action(monst_res, &pl_stats, &monst_stats);
        self.data_layer.set_monst_next_action(monst_state.db_id, &next_action).await.map_err(|e| e.into())?;
                
        return Ok(RoundResult::Next { pl_stats, monst_stats, next_action, pl_dmg_dealt, monst_dmg_dealt, monst_pow_used });
    }

    fn get_monster_next_action(&self, monst_res: &Monster, pl_stats: &Stats, monst_stats: &Stats) -> NextAction {
        let next_action_idx = AI[monst_res.name.as_str()].next_act(&pl_stats, &monst_stats);
        let next_flv_text = self.get_action_flv_txt(&monst_stats, &monst_res, next_action_idx).to_string();
        NextAction::new(next_action_idx, monst_stats.power, next_flv_text)
    }
}