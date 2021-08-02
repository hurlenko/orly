mod zip;

use std::collections::HashSet;
use std::ffi::CStr;
use std::os::raw::c_char;

use crate::client::{Authenticated, OreillyClient};
use crate::error::{OrlyError, Result};
use crate::models::{Chapter, ChapterMeta};
use crate::templates::{BaseHtml, ContainerXml};
use anyhow::Context;
use askama::Template;
use libxml::bindings::{
    xmlBufferContent, xmlBufferCreate, xmlBufferFree, xmlNodeDump, xmlSaveOption_XML_SAVE_AS_XML,
};
use libxml::readonly::RoNode;
use libxml::tree::Document;
use libxml::xpath::{Context as XpathContext, Object};
use reqwest::Url;

pub fn ronode_to_string(doc: &Document, node: &RoNode) -> String {
    unsafe {
        // allocate a buffer to dump into
        let buf = xmlBufferCreate();

        // dump the node
        xmlNodeDump(
            buf,
            doc.doc_ptr(),
            node.node_ptr(),
            1,                             // level of indentation
            xmlSaveOption_XML_SAVE_AS_XML as i32, /* disable formatting */
        );
        let result = xmlBufferContent(buf);
        let c_string = CStr::from_ptr(result as *const c_char);
        let node_string = c_string.to_string_lossy().into_owned();
        xmlBufferFree(buf);

        node_string
    }
}

fn extract_html_resources(document: &Document) -> Result<()> {
    let context: XpathContext = XpathContext::new(&document)
        .map_err(OrlyError::XpathError)
        .context("Failed to create xpath context")?;
    let result: Object = context
        .evaluate("//div[@id='sbo-rt-content']")
        .map_err(OrlyError::XpathError)
        .context("Failed to evaluate xpath")?;

    // let document = Html::parse_document(body);
    // let selector = Selector::parse(r#"div[id="sbo-rt-content"]"#).unwrap();
    // let content = document.select(&selector).next().unwrap();
    // println!("{}", content.inner_html());
    let body = result.get_readonly_nodes_as_vec();
    assert_eq!(body.len(), 1);

    println!("{}", ronode_to_string(&document, &body[0]));

    Ok(())
}

pub async fn parse_chapters(
    client: &OreillyClient<Authenticated>,
    chapters: Vec<Chapter>,
) -> Result<()> {
    let mut images: HashSet<Url> = HashSet::new();
    let mut stylesheets: HashSet<Url> = HashSet::new();

    for chapter in &chapters {
        println!("{}", chapter.meta().content_url);

        let base_url = &chapter.meta().asset_base_url;
        images.extend(
            chapter
                .meta()
                .images
                .iter()
                .map(|x| base_url.join(x))
                .collect::<std::result::Result<Vec<Url>, _>>()
                .context("Failed to join image url")?,
        );
        stylesheets.extend(chapter.meta().stylesheets.iter().map(|x| x.url.clone()));
        stylesheets.extend(chapter.meta().site_styles.iter().cloned());
        extract_html_resources(&chapter.content())?;
        break;
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
