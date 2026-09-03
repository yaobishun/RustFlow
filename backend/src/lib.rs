pub mod auth;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;
pub mod services;

use axum::Router;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub jwt_secret: String,
}

pub fn app(state: AppState) -> Router {
    routes::router(state)
}
