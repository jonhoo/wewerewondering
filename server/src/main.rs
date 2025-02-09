use lambda_http::Error;
use tracing_subscriber::EnvFilter;
use wewerewondering_api::new;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let app = new().await;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    Ok(axum::serve(listener, app.into_make_service()).await?)
}
