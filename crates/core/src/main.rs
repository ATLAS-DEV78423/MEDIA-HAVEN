#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    tracing::info!("mediahaven-core starting");
}
