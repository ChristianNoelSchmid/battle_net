use crate::data_layer_error::DataLayerError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ItemsServiceError>;

#[derive(Error, Debug)]
pub enum ItemsServiceError {
    #[error("Database error: {0}")]
    DataLayerError(#[from] DataLayerError),
    #[error("No unequipped item found in inventory")]
    NoUnequippedItem,
    #[error("Item not found")]
    ItemNotFound,
    #[error("Item already equipped")]
    ItemAlreadyEquipped,
    #[error("Item not equipped")]
    ItemNotEquipped,
    #[error("User does not own this item")]
    ItemNotOwned,
    #[error("Invalid item index")]
    InvalidItemIndex,
    #[error("Cannot consume equipped item")]
    CannotConsumeEquippedItem,
}