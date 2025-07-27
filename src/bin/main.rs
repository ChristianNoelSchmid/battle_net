use std::{fs, sync::Arc, net::SocketAddr};

use axum::Router;
use dotenvy::dotenv;
use dotenv_codegen::dotenv;

use lazy_static::lazy_static;

use christmas_2022::{
    resources::game_resources::{ResourceLoader, Resources}, 
    services::{
        token_service::{settings::TokenSettings, CoreTokenService}, 
        auth_service::{data_layer::DbAuthDataLayer, CoreAuthService}, 
        game_service::{data_layer::DbGameDataLayer, DbGameService}, 
        quest_service::{data_layer::DbQuestDataLayer, CoreQuestService}, 
        battle_service::{CoreBattleService, data_layer::DataLayer},
        items_service::{CoreItemsService, data_layer::DbDataLayer as ItemsDbDataLayer},
    }, 
    routes::{auth_routes, game_routes, quest_routes, battle_routes, items_routes}, 
    background_svcs::user_background_svc::{create_refresh_job, self},
};
use sqlx::SqlitePool;
use tokio_cron_scheduler::JobScheduler;
use tower_cookies::CookieManagerLayer;
use tower_http::trace::{TraceLayer, self};
use tracing::Level;

lazy_static! {
    static ref DATABASE_URL: &'static str = {
        dotenv().ok();
        dotenv!("DATABASE_URL")
    };
}

#[tokio::main]
async fn main() {
    // Setup tracing_subscriber
    tracing_subscriber::fmt().with_target(false).compact().init();

    // Setup state
    let db = SqlitePool::connect(&DATABASE_URL).await.unwrap();
    let token_settings: TokenSettings = serde_json::from_str(&fs::read_to_string("./token_settings.json").unwrap()).unwrap();
    let user_backround_svc_settings: user_background_svc::settings::Settings = serde_json::from_str(&fs::read_to_string("./daily_refresh.json").unwrap()).unwrap();
    let token_service = Arc::new(CoreTokenService::new(token_settings.clone()));
    let res = Arc::new(Resources::from_loader(ResourceLoader::load(String::from("./res"))));
    
    let auth_data_layer = Arc::new(DbAuthDataLayer::new(db.clone(), token_settings.clone()));
    let auth_service = Arc::new(CoreAuthService::new(auth_data_layer.clone(), token_service.clone())); 

    let game_data_layer = Arc::new(DbGameDataLayer::new(db.clone()));
    let game_service = Arc::new(DbGameService::new(game_data_layer, auth_service.clone(), res.clone()));

    let items_data_layer = Arc::new(ItemsDbDataLayer::new(db.clone()));
    let items_service = Arc::new(CoreItemsService::new(items_data_layer, res.clone()));

    let quest_data_layer = Arc::new(DbQuestDataLayer::new(db.clone()));
    let quest_service = Arc::new(CoreQuestService::new(quest_data_layer, res.clone(), game_service.clone(), items_service.clone()));

    let battle_data_layer = Arc::new(DataLayer::new(db.clone()));
    let battle_service = Arc::new(CoreBattleService::new(battle_data_layer, quest_service.clone(), items_service.clone()));

    let app = Router::new()
        // Routes
        .nest("/api/v1/auth", auth_routes::routes(auth_service))
        .nest("/api/v1/game", game_routes::routes(game_service, token_service.clone()))
        .nest("/api/v1/quest", quest_routes::routes(quest_service.clone(), token_service.clone()))
        .nest("/api/v1/battle", battle_routes::routes(token_service, quest_service, battle_service))
        .nest("/api/v1/items", items_routes::routes(token_service.clone(), items_service.clone()))
        // Logging
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO))
        )
        // Cookies
        .layer(CookieManagerLayer::new());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3005)); 

    // User background refresh stats service
    let sched = JobScheduler::new().await.unwrap();
    let usr_svc_data_layer = Arc::new(user_background_svc::data_layer::DbDataLayer { db: db.clone() });
    sched.add(create_refresh_job(usr_svc_data_layer, res, user_backround_svc_settings).unwrap()).await.unwrap();
    tokio::spawn(async move { sched.start().await.unwrap() });

    axum::Server::bind(&addr)
        .serve(app.into_make_service()).await.unwrap();
}