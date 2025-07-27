use axum::async_trait;
use sqlx::{SqlitePool, Row};
use crate::services::game_service::models::Stats;
use crate::data_layer_error::DataLayerError;

use super::error::Result;

#[async_trait]
pub trait DataLayer : Send + Sync {
    async fn get_user_stats_id(&self, user_id: i32) -> Result<i32>;
    async fn get_user_stats(&self, user_id: i32) -> Result<Stats>;
    async fn get_user_items(&self, user_id: i32) -> Result<Vec<(i32, i32)>>; // (item_id, item_idx)
    async fn get_equipped_items(&self, user_id: i32) -> Result<Vec<i32>>; // item_ids
    async fn add_item_to_user(&self, user_id: i32, item_idx: i32) -> Result<i32>; // returns item_id
    async fn equip_item(&self, item_id: i32) -> Result<()>;
    async fn unequip_item(&self, item_id: i32) -> Result<()>;
    async fn remove_item(&self, item_id: i32) -> Result<()>;
    async fn update_stats_by_id(&self, stats_id: i32, update_query: &str) -> Result<()>;
}

pub struct DbDataLayer {
    pub db: SqlitePool,
}

impl DbDataLayer {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DataLayer for DbDataLayer {
    async fn get_user_stats_id(&self, user_id: i32) -> Result<i32> {
        let stats_id = sqlx::query_scalar::<_, i32>(
            "SELECT stats_id FROM users WHERE id = ?"
        )
        .bind(user_id)
        .fetch_one(&self.db)
        .await
        .map_err(DataLayerError::from)?;
        
        Ok(stats_id)
    }

    async fn get_user_stats(&self, user_id: i32) -> Result<Stats> {
        let stats = sqlx::query_as::<_, Stats>(
            "SELECT stats.* FROM stats 
             JOIN users ON stats.id = users.stats_id 
             WHERE users.id = ?"
        )
        .bind(user_id)
        .fetch_one(&self.db)
        .await
        .map_err(DataLayerError::from)?;
        
        Ok(stats)
    }

    async fn get_user_items(&self, user_id: i32) -> Result<Vec<(i32, i32)>> {
        let items = sqlx::query(
            "SELECT id, item_idx FROM user_items WHERE user_id = ?"
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await
        .map_err(DataLayerError::from)?
        .into_iter()
        .map(|row| {
            let id: i32 = row.get("id");
            let item_idx: i32 = row.get("item_idx");
            (id, item_idx)
        })
        .collect();
        
        Ok(items)
    }

    async fn get_equipped_items(&self, user_id: i32) -> Result<Vec<i32>> {
        let equipped_items = sqlx::query_scalar::<_, i32>(
            "SELECT uei.item_id FROM user_equipped_items uei
             JOIN user_items ui ON uei.item_id = ui.id
             WHERE ui.user_id = ?"
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await
        .map_err(DataLayerError::from)?;
        
        Ok(equipped_items)
    }

    async fn add_item_to_user(&self, user_id: i32, item_idx: i32) -> Result<i32> {
        let item_id = sqlx::query_scalar::<_, i32>(
            "INSERT INTO user_items (user_id, item_idx) VALUES (?, ?) RETURNING id"
        )
        .bind(user_id)
        .bind(item_idx)
        .fetch_one(&self.db)
        .await
        .map_err(DataLayerError::from)?;
        
        Ok(item_id)
    }

    async fn equip_item(&self, item_id: i32) -> Result<()> {
        // Check if already equipped
        let exists = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM user_equipped_items WHERE item_id = ?"
        )
        .bind(item_id)
        .fetch_one(&self.db)
        .await
        .map_err(DataLayerError::from)?;
        
        if exists > 0 {
            return Err(super::error::ItemsServiceError::ItemAlreadyEquipped);
        }

        sqlx::query("INSERT INTO user_equipped_items (item_id) VALUES (?)")
            .bind(item_id)
            .execute(&self.db)
            .await
            .map_err(DataLayerError::from)?;
        
        Ok(())
    }

    async fn unequip_item(&self, item_id: i32) -> Result<()> {
        let rows_affected = sqlx::query("DELETE FROM user_equipped_items WHERE item_id = ?")
            .bind(item_id)
            .execute(&self.db)
            .await
            .map_err(DataLayerError::from)?
            .rows_affected();
        
        if rows_affected == 0 {
            return Err(super::error::ItemsServiceError::ItemNotEquipped);
        }
        
        Ok(())
    }

    async fn remove_item(&self, item_id: i32) -> Result<()> {
        // First remove from equipped items if it's equipped
        sqlx::query("DELETE FROM user_equipped_items WHERE item_id = ?")
            .bind(item_id)
            .execute(&self.db)
            .await
            .map_err(DataLayerError::from)?;
        
        // Then remove from user items
        let rows_affected = sqlx::query("DELETE FROM user_items WHERE id = ?")
            .bind(item_id)
            .execute(&self.db)
            .await
            .map_err(DataLayerError::from)?
            .rows_affected();
        
        if rows_affected == 0 {
            return Err(super::error::ItemsServiceError::ItemNotFound);
        }
        
        Ok(())
    }
    
    async fn update_stats_by_id(&self, stats_id: i32, update_query: &str) -> Result<()> {
        let full_query = format!("UPDATE stats SET {} WHERE id = ?", update_query);
        
        sqlx::query(&full_query)
            .bind(stats_id)
            .execute(&self.db)
            .await
            .map_err(DataLayerError::from)?;
        
        Ok(())
    }
}