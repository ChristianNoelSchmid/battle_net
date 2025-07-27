use axum::async_trait;
use sqlx::SqlitePool;
use crate::data_layer_error::DataLayerError;

#[async_trait]
pub trait DataLayer: Send + Sync {
    async fn update_stats_by_id(&self, stats_id: i32, update_query: &str) -> Result<(), DataLayerError>;
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
    async fn update_stats_by_id(&self, stats_id: i32, update_query: &str) -> Result<(), DataLayerError> {
        let full_query = format!("UPDATE stats SET {} WHERE id = ?", update_query);
        
        sqlx::query(&full_query)
            .bind(stats_id)
            .execute(&self.db)
            .await
            .map_err(|_| DataLayerError::DatabaseError)?;

        Ok(())
    }
}