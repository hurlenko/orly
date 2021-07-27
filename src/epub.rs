use std::collections::HashSet;

use crate::client::{Authenticated, OreillyClient};
use crate::error::Result;
use crate::models::{Chapter, ChapterMeta};
use crate::templates::{BaseHtml, ContainerXml};
use anyhow::Context;
use askama::Template;
use reqwest::Url;
use scraper::{Html, Selector};

fn extract_html_resources(body: &String) -> Result<()> {
    let document = Html::parse_document(body);
    let selector = Selector::parse(r#"div[id="sbo-rt-content"]"#).unwrap();
    let content = document.select(&selector).next().unwrap();
    println!("{}", content.inner_html());
    Ok(())
}

pub async fn parse_chapters(
    client: &OreillyClient<Authenticated>,
    chapters: Vec<Chapter>,
) -> Result<()> {
    let mut images: HashSet<Url> = HashSet::new();
    let mut stylesheets: HashSet<Url> = HashSet::new();

    for chapter in &chapters {
        let base_url = &chapter.meta.asset_base_url;
        images.extend(
            chapter
                .meta
                .images
                .iter()
                .map(|x| base_url.join(x))
                .collect::<std::result::Result<Vec<Url>, _>>()
                .context("Failed to join image url")?,
        );
        stylesheets.extend(chapter.meta.stylesheets.iter().map(|x| x.url.clone()));
        stylesheets.extend(chapter.meta.site_styles.iter().cloned());
        extract_html_resources(&chapter.content);
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
