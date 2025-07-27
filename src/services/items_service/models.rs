use serde::{Deserialize, Serialize};
use crate::resources::game_resources::ItemType;

#[derive(Debug, Serialize, Deserialize)]
pub struct UserItemModel {
    pub id: i32,
    pub item_idx: i32,
    pub name: String,
    pub flavor_text: String,
    pub item_type: ItemType,
    pub img_path: Option<String>,
    pub is_equipped: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryModel {
    pub items: Vec<UserItemModel>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryResponse {
    pub success: bool,
    pub inventory: Option<InventoryModel>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemActionResponse {
    pub success: bool,
    pub message: Option<String>,
    pub equipped: Option<bool>, // For equip/unequip responses
}