use rustflow_backend::{AppState, app, db};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rustflow_backend=info,tower_http=info".into()),
        )
        .init();
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://rustflow.db".into());
    let jwt_secret =
        std::env::var("JWT_SECRET").unwrap_or_else(|_| "rustflow-dev-secret-change-me".into());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let pool = db::connect(&database_url).await?;
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("RustFlow API running at http://{bind_addr}/api");
    axum::serve(
        listener,
        app(AppState {
            db: pool,
            jwt_secret,
        }),
    )
    .await?;
    Ok(())
}
