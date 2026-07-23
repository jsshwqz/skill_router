use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    aion_forge_cli::main_entry().await
}
