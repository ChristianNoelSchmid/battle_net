mod error;
pub mod data_layer;
pub mod models;

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use axum::async_trait;

use crate::resources::game_resources::{Resources, ItemType};

use self::{error::Result, data_layer::DataLayer, models::{UserItemModel, InventoryModel}};

use super::game_service::models::Stats;

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemUseResult {
    pub success: bool,
    pub message: String,
    pub stats_changed: bool,
    pub item_consumed: bool,
    pub new_stats: Option<Stats>,
}

#[async_trait]
pub trait ItemsService : Send + Sync {
    /// Get all items owned by a user
    async fn get_user_inventory(&self, user_id: i32) -> Result<InventoryModel>;
    
    /// Equip or unequip an item
    async fn toggle_equip_item(&self, user_id: i32, item_id: i32) -> Result<bool>; // returns new equipped state
    
    /// Use an item (consume if consumable, or equip if weapon/armor)
    async fn use_item(&self, user_id: i32, item_id: i32) -> Result<ItemUseResult>;
    
    /// Add an item to user's inventory
    async fn add_item_to_user(&self, user_id: i32, item_idx: i32) -> Result<i32>; // returns item_id
}

pub struct CoreItemsService {
    data_layer: Arc<dyn DataLayer>,
    resources: Arc<Resources>,
}

impl CoreItemsService {
    pub fn new(
        data_layer: Arc<dyn DataLayer>,
        resources: Arc<Resources>,
    ) -> Self {
        Self {
            data_layer,
            resources,
        }
    }
}

#[async_trait]
impl ItemsService for CoreItemsService {
    async fn get_user_inventory(&self, user_id: i32) -> Result<InventoryModel> {
        let user_items = self.data_layer.get_user_items(user_id).await?;
        let equipped_items = self.data_layer.get_equipped_items(user_id).await?;
        
        let mut inventory_items = Vec::new();
        
        for (item_id, item_idx) in user_items {
            if let Some(item) = self.resources.items.get(item_idx as usize) {
                let is_equipped = equipped_items.contains(&item_id);
                inventory_items.push(UserItemModel {
                    id: item_id,
                    item_idx,
                    name: item.name.clone(),
                    flavor_text: item.flavor_text.clone(),
                    item_type: item.item_type,
                    img_path: item.img_path.clone(),
                    is_equipped,
                });
            }
        }
        
        Ok(InventoryModel { items: inventory_items })
    }

    async fn toggle_equip_item(&self, user_id: i32, item_id: i32) -> Result<bool> {
        // Check if user owns the item
        let user_items = self.data_layer.get_user_items(user_id).await?;
        let owned_item = user_items.iter().find(|(id, _)| *id == item_id);
        
        if owned_item.is_none() {
            return Err(error::ItemsServiceError::ItemNotOwned);
        }

        let equipped_items = self.data_layer.get_equipped_items(user_id).await?;
        let is_equipped = equipped_items.contains(&item_id);
        
        if is_equipped {
            self.data_layer.unequip_item(item_id).await?;
            Ok(false)
        } else {
            self.data_layer.equip_item(item_id).await?;
            Ok(true)
        }
    }

    async fn use_item(&self, user_id: i32, item_id: i32) -> Result<ItemUseResult> {
        // Check if user owns the item
        let user_items = self.data_layer.get_user_items(user_id).await?;
        let owned_item = user_items.iter().find(|(id, _)| *id == item_id);
        
        let (_, item_idx) = owned_item.ok_or(error::ItemsServiceError::ItemNotOwned)?;
        
        let item = self.resources.items.get(*item_idx as usize)
            .ok_or(error::ItemsServiceError::InvalidItemIndex)?;

        match item.item_type {
            ItemType::Consumable => {
                // Check if item is equipped (consumables can't be used if equipped)
                let equipped_items = self.data_layer.get_equipped_items(user_id).await?;
                if equipped_items.contains(&item_id) {
                    return Err(error::ItemsServiceError::CannotConsumeEquippedItem);
                }

                // Apply effects if any
                let mut stats_changed = false;
                let mut new_stats = None;
                
                if let Some(effects) = &item.effects_self {
                    let stats_id = self.data_layer.get_user_stats_id(user_id).await?;
                    
                    for (effect_name, value) in effects {
                        match effect_name.as_str() {
                            "boost_health" => {
                                let update_query = format!("health = health + {}", value);
                                self.data_layer.update_stats_by_id(stats_id, &update_query).await
                                    .map_err(|e| error::ItemsServiceError::DataLayerError(e.into()))?;
                                stats_changed = true;
                            },
                            "boost_armor" => {
                                let update_query = format!("armor = armor + {}", value);
                                self.data_layer.update_stats_by_id(stats_id, &update_query).await
                                    .map_err(|e| error::ItemsServiceError::DataLayerError(e.into()))?;
                                stats_changed = true;
                            },
                            "boost_power" => {
                                let update_query = format!("power = power + {}", value);
                                self.data_layer.update_stats_by_id(stats_id, &update_query).await
                                    .map_err(|e| error::ItemsServiceError::DataLayerError(e.into()))?;
                                stats_changed = true;
                            },
                            _ => {}
                        }
                    }
                    
                    if stats_changed {
                        new_stats = Some(self.data_layer.get_user_stats(user_id).await?);
                    }
                }

                // Remove the item (consumed)
                self.data_layer.remove_item(item_id).await?;

                Ok(ItemUseResult {
                    success: true,
                    message: format!("Used {} successfully!", item.name),
                    stats_changed,
                    item_consumed: true,
                    new_stats,
                })
            },
            ItemType::Weapon(damage) => {
                // For weapons, return damage info
                Ok(ItemUseResult {
                    success: true,
                    message: format!("Used {} for {} damage!", item.name, damage),
                    stats_changed: false,
                    item_consumed: false,
                    new_stats: None,
                })
            },
            ItemType::Equipable => {
                // Toggle equip state
                let new_equipped_state = self.toggle_equip_item(user_id, item_id).await?;
                let action = if new_equipped_state { "equipped" } else { "unequipped" };
                
                Ok(ItemUseResult {
                    success: true,
                    message: format!("{} {} successfully!", action, item.name),
                    stats_changed: false,
                    item_consumed: false,
                    new_stats: None,
                })
            }
        }
    }

    async fn add_item_to_user(&self, user_id: i32, item_idx: i32) -> Result<i32> {
        self.data_layer.add_item_to_user(user_id, item_idx).await
    }
}