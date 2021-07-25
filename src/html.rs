use crate::error::Result;
use anyhow::Context;
use lol_html::{element, HtmlRewriter, Settings};
use std::fs::File;
use std::io::prelude::*;

pub fn rewrite() -> Result<()> {
    let mut output = vec![];

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!("[src], [href]", |el| {
                // let href = el
                //     .get_attribute("src")
                //     .expect("href was required")
                //     .replace("test_files", "kek");

                el.set_attribute("lol", "kek")?;

                Ok(())
            })],
            ..Settings::default()
        },
        |c: &[u8]| output.extend_from_slice(c),
    );

    rewriter.write(include_bytes!("../test.html")).context("")?;
    rewriter.end().context("")?;
    let mut file = File::create("test_res.html").context("context")?;
    // Write a slice of bytes to the file
    file.write_all(&output).context("context")?;
    Ok(())
}
