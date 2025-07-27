pub mod ai;
pub mod dice;

pub mod background_svcs {
    pub mod user_background_svc;
}

pub mod data_layer_error;

pub mod middleware {
    pub mod auth_middleware;
}

pub mod routes {
    pub mod game_routes;
    pub mod quest_routes;
    pub mod auth_routes;
    pub mod battle_routes;
    pub mod items_routes;
}

pub mod resources {
    pub mod game_resources;
}

pub mod services {
    pub mod game_service;
    pub mod auth_service;
    pub mod token_service;
    pub mod quest_service;
    pub mod battle_service;
    pub mod items_service;
}