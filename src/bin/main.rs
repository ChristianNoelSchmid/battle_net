use std::{fs, sync::Arc, net::SocketAddr};

use axum::Router;
use dotenvy::dotenv;
use dotenv_codegen::dotenv;

use lazy_static::lazy_static;

use christmas_2022::{
    resources::game_resources::{ResourceLoader, Resources}, 
    services::{token_service::{settings::TokenSettings, CoreTokenService}, 
    auth_service::{data_layer::DbAuthDataLayer, CoreAuthService}, 
    game_service::{data_layer::DbGameDataLayer, DbGameService}}, 
    routes::{auth_routes, game_routes}, prisma::PrismaClient,
};
use tower_cookies::CookieManagerLayer;
use tower_http::trace::{TraceLayer, self};
use tracing::Level;

lazy_static! {
    static ref DATABASE_URL: &'static str = {
        dotenv().unwrap();
        dotenv!("DATABASE_URL")
    };
}

#[tokio::main]
async fn main() {
    // Setup tracing_subscriber
    tracing_subscriber::fmt().with_target(false).compact().init();

    // Setup state
    let db = Arc::new(PrismaClient::_builder().build().await.unwrap());
    let token_settings: TokenSettings = serde_json::from_str(&fs::read_to_string("./token_settings.json").unwrap()).unwrap();
    let token_service = Arc::new(CoreTokenService::new(token_settings.clone()));
    let res = Arc::new(Resources::from_loader(ResourceLoader::load(String::from("./res"))));

    let auth_data_layer = Arc::new(DbAuthDataLayer::new(db.clone(), token_settings.clone()));
    let auth_service = Arc::new(CoreAuthService::new(auth_data_layer.clone(), token_service.clone())); 

    let game_data_layer = Arc::new(DbGameDataLayer::new(db));
    let game_service = Arc::new(DbGameService::new(game_data_layer, auth_service.clone(), res.clone()));

    let app = Router::new()
        // Routes
        .nest("/auth", auth_routes::routes(auth_service))
        .nest("/game", game_routes::routes(game_service, token_service))
        // Logging
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO))
        )
        // Cookies
        .layer(CookieManagerLayer::new());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3005));
    axum::Server::bind(&addr)
        .serve(app.into_make_service()).await.unwrap();
}
