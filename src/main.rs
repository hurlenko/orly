pub mod client;
pub mod error;
pub mod models;

use error::Result;

use crate::client::OreillyClient;

async fn run() -> Result<()> {
    let book_id = "0735619670";
    let client = OreillyClient::new();
    client
        .cred_auth(
            "limowij820@godpeed.com".to_string(),
            "qwerty123".to_string(),
        )
        .await?;

    client.check_login().await?;

    let book = client.fetch_book_deails(book_id.to_string()).await?;
    println!("{:#?}", book);

    let chapters = client.fetch_book_chapters(book_id.to_string()).await?;

    println!("Downloaded {} chapters", chapters.len());

    let toc = client.fetch_toc(book_id).await?;

    println!("Downloaded toc: {:?}", toc.len());


    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(err) = run().await {
        eprintln!("{:?}", err)
    }

    Ok(())
}
