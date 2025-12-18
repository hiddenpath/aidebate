mod app_metrics;
mod config;
mod debate;
mod handlers;
mod prompts;
mod storage;
mod types;

use axum::Router;
use tokio::net::TcpListener;

use crate::config::{init_clients, init_tracing};
use crate::handlers::{build_app, serve};
use crate::storage::init_db;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://debate.db".to_string());
    let db = init_db(&db_url).await?;

    let clients = init_clients()?;
    let app: Router = build_app(db, clients).await;

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    serve(listener, app).await
}


