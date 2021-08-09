mod builder;
mod zip;

use tokio::fs::File;

use anyhow::Context;

use crate::client::{Authenticated, OreillyClient};
use crate::epub::builder::EpubBuilder;
use crate::error::Result;

pub async fn build_epub(client: OreillyClient<Authenticated>, book_id: &str) -> Result<()> {
    let book = client.fetch_book_deails(book_id).await?;
    println!("{:#?}", book);

    let chapters = client.fetch_book_chapters(book_id).await?;

    println!("Downloaded {} chapters", chapters.len());

    // Todo use tokio io;
    let file = File::create("epub.zip")
        .await
        .context("Unable to create file")?;

    let mut epub = EpubBuilder::new()?;
    epub.chapters(chapters)?.generate(file).await?;

    // parse_chapters(&client, chapters).await?;

    // for (idx, chapter) in chapters.iter().enumerate() {
    //     println!("{} {}", idx, chapter.content_url);
    // }

    // let toc = client.fetch_toc(book_id).await?;

    // println!("Downloaded toc: {:?}", toc.len());
    Ok(())
}
