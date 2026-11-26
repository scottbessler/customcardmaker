#[tokio::main]
async fn main() -> anyhow::Result<()> {
    customcardmaker::app::run().await
}
