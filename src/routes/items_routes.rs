use std::sync::Arc;

use axum::{
    Router, 
    routing::{get, post}, 
    extract::{FromRef, State, Path}, 
    response::IntoResponse, 
    Json,
    http::StatusCode,
    http::HeaderMap,
};

use crate::services::{
    token_service::TokenService,
    items_service::{ItemsService, models::{InventoryResponse, ItemActionResponse}}
};

#[derive(Clone, FromRef)]
pub struct ItemsRoutesState {
    token_service: Arc<dyn TokenService>,
    items_service: Arc<dyn ItemsService>,
}

pub fn routes(
    token_service: Arc<dyn TokenService>, 
    items_service: Arc<dyn ItemsService>
) -> Router {
    Router::new()
        .route("/inventory", get(get_inventory))
        .route("/equip/:item_id", post(toggle_equip_item))
        .route("/use/:item_id", post(use_item))
        .route("/give/:item_idx", post(give_item_to_user)) // For testing
        .with_state(ItemsRoutesState {
            token_service,
            items_service,
        })
}

fn extract_token(headers: &HeaderMap) -> Option<String> {
    let auth_header = headers.get("Authorization")?;
    let auth_str = auth_header.to_str().ok()?;
    if auth_str.starts_with("Bearer ") {
        Some(auth_str.replace("Bearer ", ""))
    } else {
        None
    }
}

/// Get user's inventory
async fn get_inventory(
    State(token_service): State<Arc<dyn TokenService>>,
    State(items_service): State<Arc<dyn ItemsService>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = match extract_token(&headers) {
        Some(token) => token,
        None => return Json(InventoryResponse {
            success: false,
            inventory: None,
            message: Some("Missing authorization token".to_string()),
        }).into_response(),
    };

    let user_id = match token_service.decode_token(&token).await {
        Ok(claims) => claims.user_id,
        Err(_) => return Json(InventoryResponse {
            success: false,
            inventory: None,
            message: Some("Invalid token".to_string()),
        }).into_response(),
    };

    match items_service.get_user_inventory(user_id as i32).await {
        Ok(inventory) => Json(InventoryResponse {
            success: true,
            inventory: Some(inventory),
            message: None,
        }).into_response(),
        Err(e) => {
            eprintln!("Error getting inventory: {:?}", e);
            Json(InventoryResponse {
                success: false,
                inventory: None,
                message: Some("Failed to get inventory".to_string()),
            }).into_response()
        }
    }
}

/// Toggle equip/unequip an item
async fn toggle_equip_item(
    State(token_service): State<Arc<dyn TokenService>>,
    State(items_service): State<Arc<dyn ItemsService>>,
    Path(item_id): Path<i32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = match extract_token(&headers) {
        Some(token) => token,
        None => return Json(ItemActionResponse {
            success: false,
            message: Some("Missing authorization token".to_string()),
            equipped: None,
        }).into_response(),
    };

    let user_id = match token_service.decode_token(&token).await {
        Ok(claims) => claims.user_id,
        Err(_) => return Json(ItemActionResponse {
            success: false,
            message: Some("Invalid token".to_string()),
            equipped: None,
        }).into_response(),
    };

    match items_service.toggle_equip_item(user_id as i32, item_id).await {
        Ok(new_equipped_state) => Json(ItemActionResponse {
            success: true,
            message: Some(format!("Item {} successfully", if new_equipped_state { "equipped" } else { "unequipped" })),
            equipped: Some(new_equipped_state),
        }).into_response(),
        Err(e) => {
            eprintln!("Error toggling item equipment: {:?}", e);
            Json(ItemActionResponse {
                success: false,
                message: Some("Failed to toggle item equipment".to_string()),
                equipped: None,
            }).into_response()
        }
    }
}

/// Use/consume an item
async fn use_item(
    State(token_service): State<Arc<dyn TokenService>>,
    State(items_service): State<Arc<dyn ItemsService>>,
    Path(item_id): Path<i32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let auth_header = headers.get("Authorization");
    
    if auth_header.is_none() {
        return (StatusCode::UNAUTHORIZED, Json(ItemActionResponse {
            success: false,
            message: "Missing authorization header".to_string(),
            stats_changed: false,
            new_stats: None,
        })).into_response();
    }

    let token = auth_header.unwrap().to_str().unwrap().replace("Bearer ", "");
    
    let user_id = match token_service.verify_access_token(&token) {
        Ok(id) => id,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(ItemActionResponse {
            success: false,
            message: "Invalid token".to_string(),
            stats_changed: false,
            new_stats: None,
        })).into_response(),
    };

    match items_service.use_item(user_id as i32, item_id).await {
        Ok(result) => (StatusCode::OK, Json(ItemActionResponse {
            success: result.success,
            message: result.message,
            stats_changed: result.stats_changed,
            new_stats: result.new_stats,
        })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ItemActionResponse {
            success: false,
            message: format!("Failed to use item: {}", e),
            stats_changed: false,
            new_stats: None,
        })).into_response(),
    }
}

/// Give an item to the user (admin/quest reward endpoint)
async fn give_item_to_user(
    State(token_service): State<Arc<dyn TokenService>>,
    State(items_service): State<Arc<dyn ItemsService>>,
    Path(item_idx): Path<i32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = match extract_token(&headers) {
        Some(token) => token,
        None => return Json(ItemActionResponse {
            success: false,
            message: Some("Missing authorization token".to_string()),
            equipped: None,
        }).into_response(),
    };

    let user_id = match token_service.decode_token(&token).await {
        Ok(claims) => claims.user_id,
        Err(_) => return Json(ItemActionResponse {
            success: false,
            message: Some("Invalid token".to_string()),
            equipped: None,
        }).into_response(),
    };

    match items_service.add_item_to_user(user_id as i32, item_idx).await {
        Ok(item_id) => Json(ItemActionResponse {
            success: true,
            message: Some(format!("Item added successfully with ID: {}", item_id)),
            equipped: None,
        }).into_response(),
        Err(e) => {
            eprintln!("Error giving item to user: {:?}", e);
            Json(ItemActionResponse {
                success: false,
                message: Some("Failed to give item to user".to_string()),
                equipped: None,
            }).into_response()
        }
    }
}