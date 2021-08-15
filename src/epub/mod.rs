mod builder;
mod lxml;
mod zip;

use tokio::fs::File;

use anyhow::Context;

use crate::{
    client::{Authenticated, OreillyClient},
    epub::builder::EpubBuilder,
    error::Result,
};

pub async fn build_epub(client: OreillyClient<Authenticated>, book_id: &str) -> Result<()> {
    let book = client.fetch_book_deails(book_id).await?;
    println!("{:#?}", book);

    let chapters = client.fetch_book_chapters(book_id).await?;

    println!("Downloaded {} chapters", chapters.len());

    // Todo use tokio io;
    let file = File::create("epub.zip")
        .await
        .context("Unable to create file")?;
    let toc = client.fetch_toc(book_id).await?;
    println!("Downloaded toc: {:?}", toc.len());

    let mut epub = EpubBuilder::new(&book)?;
    epub.chapters(&chapters)?.toc(&toc)?.generate(file).await?;

    // parse_chapters(&client, chapters).await?;

    // for (idx, chapter) in chapters.iter().enumerate() {
    //     println!("{} {}", idx, chapter.content_url);
    // }

    Ok(())
}
