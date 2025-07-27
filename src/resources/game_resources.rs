use serde::{Deserialize, Serialize};
use serde_json;
use std::{fs, path::PathBuf};
use std::collections::HashMap;

#[derive(Default)]
pub struct ResourceLoader {
    pub evd_card_cats: Vec<EvidenceCardCategories>,
    pub avatars: Vec<Avatar>,
    pub monsters: Vec<Monster>,
    pub riddles: Vec<Riddle>,
    pub user_base_stats: BaseStats,
    pub items: Vec<Item>,
    // pub spells: Vec<Spell>,
}
impl ResourceLoader {
    pub fn load(folder_path: String) -> Self {
        let evd_card_cats =
            serde_json::from_str(&Self::get_file_str(&folder_path, "category_cards.json"))
                .expect("Could not parse file into category cards");
        let avatars = serde_json::from_str(&Self::get_file_str(&folder_path, "avatars.json"))
            .expect("Could not parse file into avatars");
        let monsters = serde_json::from_str(&Self::get_file_str(&folder_path, "monsters.json"))
            .expect("Could not parse file into monsters");
        let riddles = serde_json::from_str(&Self::get_file_str(&folder_path, "riddles.json"))
            .expect("Could not parse file into riddles");
        let user_base_stats =
            serde_json::from_str(&Self::get_file_str(&folder_path, "user_base_stats.json"))
                .expect("Could not parse file into user base stats");
        let items = serde_json::from_str(&Self::get_file_str(&folder_path, "items.json"))
            .expect("Could not parse file into items");
        /*let spells = serde_json::from_str(&Self::get_file_str(&folder_path, "spells.json"))
            .expect("Could not parse file into spells");*/

        Self {
            evd_card_cats,
            avatars,
            monsters,
            riddles,
            user_base_stats,
            items
            // items,
            // spells,
        }
    }
    fn get_file_str(folder_path: &str, file_name: &str) -> String {
        let mut path = PathBuf::from(folder_path);
        path.push(file_name);

        fs::read_to_string(path).expect("Could not load resource path")
    }
}

#[derive(Clone)]
pub struct Resources {
    pub evd_cats_and_cards: Vec<EvidenceCardCategories>,
    pub avatars: Vec<Avatar>,
    pub monsters: Vec<Monster>,
    pub riddles: Vec<Riddle>,
    pub items: Vec<Item>,
    // pub spells: Vec<Spell>,
    pub user_base_stats: BaseStats,
}

impl Resources {
    pub fn from_loader(res_loader: ResourceLoader) -> Self {
        Self {
            evd_cats_and_cards: res_loader.evd_card_cats,
            avatars: res_loader.avatars,
            monsters: res_loader.monsters,
            riddles: res_loader.riddles,
            user_base_stats: res_loader.user_base_stats,
            items: res_loader.items,
            // spells: res_loader.spells,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Avatar {
    pub card_idx: i64,
    pub user_name: String,
    pub user_img_path: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EvidenceCardCategories {
    pub name: String,
    pub tag: String,
    pub cards: Vec<EvidenceCard>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EvidenceCard {
    pub name: String,
    pub img_path: Option<String>,
    pub flavor_text: Option<String>,
}
#[derive(Clone, Deserialize)]
pub struct SabotageCard {
    pub name: String,
    pub effect_tags: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Monster {
    pub name: String,
    pub level: i64,
    pub stats: BaseStats,
    pub pow_dmg: Vec<(i64, i64)>,

    pub attack_flv_texts: Vec<String>,
    pub defend_flv_texts: Vec<String>,
    pub idle_flv_texts: Vec<String>
}

#[derive(Default, Serialize, Deserialize, Clone, Copy)]
pub struct BaseStats {
    pub health: i64,
    pub armor: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Riddle {
    pub text: String,
    pub answer: String,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum ItemType {
    #[serde(rename = "weapon")]
    Weapon(i64),
    #[serde(rename = "equipable")]
    Equipable,
    #[serde(rename = "consumable")]
    Consumable,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    pub tag: String,
    pub name: String,
    pub flavor_text: String,
    pub item_type: ItemType,
    pub img_path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects_self: Option<HashMap<String, i64>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects_other: Option<HashMap<String, i64>>,
}

#[serde_with::serde_as]
#[derive(Serialize, Deserialize)]
pub struct Spell {
    pub tag: String,
    pub name: String,
    pub flavor_text: String,
    pub magicka_cost: i32,
    /*#[serde_as(as = "Option<EnumMap>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects_self: Option<Vec<EffectType>>,
    #[serde_as(as = "Option<EnumMap>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects_other: Option<Vec<EffectType>>,*/
}