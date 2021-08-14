
use orly::error::Result;

use orly::client::OreillyClient;
use orly::epub;

async fn run() -> Result<()> {
    let book_id = "9781492056348";
    let client = OreillyClient::new()
        .cred_auth("diwesaf782@dmsdmg.com".to_string(), "qwerty123".to_string())
        .await?;

    epub::build_epub(client, book_id).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(err) = run().await {
        eprintln!("{}", err)
    }

    Ok(())
}
