use axum::{response::{IntoResponse, Response}, http::StatusCode};
use log::error;
use thiserror::Error;

use crate::{data_layer_error::DataLayerError, services::quest_service::error::QuestServiceError};

pub type Result<T> = std::result::Result<T, BattleServiceError>;

#[derive(Debug, Error)]
pub enum BattleServiceError {
    #[error("An internal server error has occurred")]
    DataLayerError(DataLayerError),
    #[error("An internal server error has occurred")]
    QuestServiceError(QuestServiceError),
    #[error("Items service error: {0}")]
    ItemsServiceError(String),
    #[error("Quest not found for user {0}")]
    QuestNotFound(i32),
    #[error("Too much power requested. Request less.")]
    NotEnoughPower,
    #[error("Power is out of bounds (please choose from 1-4)")]
    PowerOutOfRange,
}

impl Into<BattleServiceError> for DataLayerError {
    fn into(self) -> BattleServiceError {
        BattleServiceError::DataLayerError(self)
    }
}

impl IntoResponse for BattleServiceError {
    fn into_response(self) -> Response {
        println!("{:?}", self);
        return if let BattleServiceError::DataLayerError(_) = &self {
            (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
        } else if let BattleServiceError::QuestServiceError(_) = &self {
            (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
        } else {
            (StatusCode::BAD_REQUEST, self.to_string()).into_response()
        }
    }
}

impl From<QuestServiceError> for BattleServiceError {
    fn from(err: QuestServiceError) -> Self {
        BattleServiceError::QuestServiceError(err)
    }
}
