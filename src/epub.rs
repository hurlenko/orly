use std::collections::HashSet;

use crate::client::{Authenticated, OreillyClient};
use crate::error::Result;
use crate::models::Chapter;
use crate::templates::{BaseHtml, ContainerXml};
use anyhow::Context;
use askama::Template;
use reqwest::Url;

pub async fn parse_chapters(
    client: &OreillyClient<Authenticated>,
    chapters: Vec<Chapter>,
) -> Result<()> {
    let mut images: HashSet<Url> = HashSet::new();
    let mut stylesheets: HashSet<Url> = HashSet::new();

    for chapter in &chapters {
        let base_url = &chapter.asset_base_url;
        images.extend(
            chapter
                .images
                .iter()
                .map(|x| base_url.join(x))
                .collect::<std::result::Result<Vec<Url>, _>>()
                .context("Failed to join image url")?,
        );
        stylesheets.extend(chapter.stylesheets.iter().map(|x| x.url.clone()));
        stylesheets.extend(chapter.site_styles.iter().cloned());
    }
    println!("Found {} images", images.len());
    println!("Found {} stylesheets", stylesheets.len());
    Ok(())
}

pub async fn build_epub(client: OreillyClient<Authenticated>, book_id: &str) -> Result<()> {
    // let hello = BaseHtml {
    //     styles: "world",
    //     body: "test",
    //     should_support_kindle: true,
    // };
    // println!("{}", hello.render().context("Invalid base template")?);
    // println!(
    //     "{}",
    //     ContainerXml {}.render().context("Invalid base template")?
    // );

    let book = client.fetch_book_deails(book_id).await?;
    println!("{:#?}", book);

    let chapters = client.fetch_book_chapters(book_id).await?;

    println!("Downloaded {} chapters", chapters.len());
    parse_chapters(&client, chapters).await?;

    // for (idx, chapter) in chapters.iter().enumerate() {
    //     println!("{} {}", idx, chapter.content_url);
    // }

    // let toc = client.fetch_toc(book_id).await?;

    // println!("Downloaded toc: {:?}", toc.len());
    Ok(())
}
