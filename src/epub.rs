use crate::client::{Authenticated, OreillyClient};
use crate::error::Result;
use crate::templates::{BaseHtml, ContainerXml};
use anyhow::Context;
use askama::Template;

pub async fn build_epub(client: OreillyClient<Authenticated>, book_id: &str) -> Result<()> {
    let hello = BaseHtml {
        styles: "world",
        body: "test",
        should_support_kindle: true,
    };
    println!("{}", hello.render().context("Invalid base template")?);
    println!(
        "{}",
        ContainerXml {}.render().context("Invalid base template")?
    );

    let book = client.fetch_book_deails(book_id).await?;
    println!("{:#?}", book);

    let chapters = client.fetch_book_chapters(book_id).await?;

    println!("Downloaded {} chapters", chapters.len());

    for (idx, chapter) in chapters.iter().enumerate() {
        println!("{} {}", idx, chapter.content);
    }

    // let toc = client.fetch_toc(book_id).await?;

    // println!("Downloaded toc: {:?}", toc.len());
    Ok(())
}
