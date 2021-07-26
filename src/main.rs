pub mod client;
pub mod error;
pub mod models;
pub mod templates;
mod epub;
mod html;

use anyhow::Context;
use error::Result;

use crate::client::OreillyClient;

async fn run() -> Result<()> {
    let book_id = "0735619670";
    let client = OreillyClient::new()
        .cred_auth(
            "diwesaf781@dmsdmg.com".to_string(),
            "qwerty123".to_string(),
        )
        .await?;

    epub::build_epub(client, book_id).await?;
    // html::rewrite()?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(err) = run().await {
        eprintln!("{}", err)
    }

    Ok(())
}
