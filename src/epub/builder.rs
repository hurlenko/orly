use std::collections::{HashMap, HashSet};

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use crate::{
    client::{Authenticated, OreillyClient},
    epub::lxml::DocumentExt,
    error::{OrlyError, Result},
    models::{Book, Chapter, TocElement},
    templates::{ChapterXhtml, ContainerXml, ContentOpf, IbooksXml, NavPoint, Toc},
};

use anyhow::Context;
use askama::Template;

use libxml::{parser::Parser, tree::SaveOptions};
use log::{debug, info, warn};
use reqwest::Url;
use url::ParseError;

use super::zip::ZipArchive;
use lazy_static::lazy_static;

const XHTML: &str = "xhtml";
const IMAGES: &str = "images";
const STYLES: &str = "styles";

lazy_static! {
    static ref OEBPS: PathBuf = PathBuf::from("OEBPS");
}

pub struct EpubBuilder<'a> {
    zip: ZipArchive,
    book: &'a Book,
    stylesheets: HashMap<Url, String>,
    images: HashMap<Url, String>,
    parser: Parser,
    chapter_names: Vec<&'a str>,
    // image name
    cover: String,
    kindle: bool,
}

impl<'a> EpubBuilder<'a> {
    pub fn new(book: &'a Book, kindle: bool) -> Result<Self> {
        let mut epub = EpubBuilder {
            zip: ZipArchive::new()?,
            book,
            kindle,
            parser: Parser::default_html(),
            stylesheets: Default::default(),
            images: Default::default(),
            chapter_names: Default::default(),
            cover: Default::default(),
        };

        epub.zip.write_file(
            "META-INF/container.xml",
            ContainerXml
                .render()
                .context("failed to render IbooksXml")?
                .as_bytes(),
        )?;
        epub.zip.write_file(
            "META-INF/com.apple.ibooks.display-options.xml",
            IbooksXml
                .render()
                .context("failed to render IbooksXml")?
                .as_bytes(),
        )?;

        Ok(epub)
    }

    fn rewrite_chapter_links(&self, old: &str) -> String {
        // Url does not support relative urls, use dummy host to convert to absolute
        let abs_url = match Url::parse(old) {
            Err(ParseError::RelativeUrlWithoutBase) => {
                match Url::parse("https://example.net").and_then(|base| base.join(old)) {
                    Ok(url) => url,
                    _ => return old.to_string(),
                }
            }
            _ => return old.to_string(),
        };

        let path = match PathBuf::from(abs_url.path())
            .file_name()
            .and_then(OsStr::to_str)
        {
            Some(filename) => PathBuf::from(filename),
            _ => return old.to_string(),
        };

        // For images and html create a new path
        let new_path = match path.extension().and_then(OsStr::to_str) {
            Some("png" | "jpg" | "jpeg" | "gif") => path
                .to_str()
                .map(|filename| format!("{}/{}", IMAGES, filename)),
            Some("html") => path.with_extension(XHTML).to_str().map(str::to_string),
            _ => return old.to_string(),
        };

        // Append query params and fragmets, if any
        if let Some(mut new_path) = new_path {
            if let Some(query) = abs_url.query() {
                new_path.push('?');
                new_path.push_str(query);
            }
            if let Some(fragment) = abs_url.fragment() {
                new_path.push('#');
                new_path.push_str(fragment);
            }
            return new_path;
        }
        old.to_string()
    }

    fn extract_chapter_content(&self, chapter_body: &str) -> Result<String> {
        let document = self.parser.parse_string(chapter_body)?;
        let rewritten = document.rewrite_links(|old| self.rewrite_chapter_links(old));
        debug!("Links rewritten: {}", rewritten);

        let body = document.xpath("//div[@id='sbo-rt-content']");
        if body.len() != 1 {
            return Err(OrlyError::ParseError(format!(
                "Unable to find content div in chapter: {}",
                chapter_body
            )));
        }

        Ok(document.node_to_string_with_options(
            &body[0],
            SaveOptions {
                as_xml: true,
                ..Default::default()
            },
        ))
    }

    fn extract_images(&self, chapter: &Chapter) -> Result<Vec<(Url, String)>> {
        let base_url = Url::parse(&format!(
            "https://learning.oreilly.com/api/v2/epubs/urn:orm:book:{}/files/",
            self.book.identifier
        ))
        .unwrap();

        let image_urls = chapter
            .meta
            .images
            .iter()
            .map(|x| {
                base_url.join(x).ok().and_then(|url| {
                    PathBuf::from(url.path())
                        .file_name()
                        .and_then(OsStr::to_str)
                        .map(|filename| (url, format!("{}/{}", IMAGES, filename)))
                })
            })
            .collect::<Option<Vec<_>>>()
            .context("Failed to join image url")?;

        Ok(image_urls)
    }

    fn extract_styles(&mut self, chapter: &Chapter) -> Result<()> {
        for style in chapter
            .meta
            .stylesheets
            .iter()
            .map(|x| x.url.clone())
            .chain(chapter.meta.site_styles.iter().cloned())
        {
            let count = self.stylesheets.len();
            self.stylesheets
                .entry(style)
                .or_insert(format!("{}/{}.css", STYLES, count));
        }

        Ok(())
    }

    fn add_chapter(&mut self, chapter: &Chapter) -> Result<()> {
        debug!("Processing {}", &chapter.meta.filename);
        let chapter_xhtml = ChapterXhtml {
            styles: &self.stylesheets.values().collect(),
            body: &self.extract_chapter_content(&chapter.content)?,
            should_support_kindle: self.kindle,
        };

        let filename = OEBPS.as_path().join(&chapter.meta.filename);

        self.zip.write_file(
            filename,
            chapter_xhtml
                .render()
                .context("failed to render chapter xhtml")?
                .as_bytes(),
        )?;
        Ok(())
    }

    pub fn chapters(&mut self, chapters: &'a [Chapter]) -> Result<&mut Self> {
        for chapter in chapters {
            let images = self.extract_images(chapter)?;

            if let Some("cover") = Path::new(&chapter.meta.filename.to_lowercase())
                .file_stem()
                .and_then(OsStr::to_str)
            {
                debug!("Found cover in {:?}", chapter.meta.filename);
                assert_eq!(images.len(), 1);
                self.cover = images[0].1.clone();
            }

            self.images.extend(images);
            self.extract_styles(chapter)?;

            self.chapter_names.push(&chapter.meta.filename);

            self.add_chapter(chapter)?;
        }

        info!("Found {} images", self.images.len());
        info!("Found {} stylesheets", self.stylesheets.len());
        Ok(self)
    }

    pub async fn generate<W: tokio::io::AsyncWrite + std::marker::Unpin>(
        &mut self,
        to: W,
        client: OreillyClient<Authenticated>,
    ) -> Result<()> {
        // Unique urls != unique filenames
        let images_count = self.images.len();
        let unique_images = self.images.values().collect::<HashSet<&String>>().len();
        if images_count != unique_images {
            warn!("Images have non-unique names, some of them might get overwritten");
        }

        let files: HashMap<&Url, &String> =
            self.images.iter().chain(self.stylesheets.iter()).collect();

        info!("Downloading {} files", files.len());
        for (url, bytes) in client.bulk_download_bytes(files.keys().cloned()).await? {
            self.zip
                .write_file(OEBPS.as_path().join(files.get(url).unwrap()), &*bytes)?;
        }

        info!("Rendering OPF and generating final EPUB");
        self.render_opf()?.zip.generate(to).await?;
        Ok(())
    }

    /// Render content.opf file
    fn render_opf(&mut self) -> Result<&mut Self> {
        let images_mime: Vec<(&String, String)> = self
            .images
            .iter()
            .map(|(_, f)| {
                (
                    f,
                    match PathBuf::from(f).extension().and_then(OsStr::to_str) {
                        Some(ext) if ext.starts_with("jp") => "jpeg",
                        Some(ext) => ext,
                        None => "png",
                    }
                    .to_string(),
                )
            })
            .collect();

        let content_opf = ContentOpf {
            title: &self.book.title,
            description: &self.book.description,
            publishers: &self
                .book
                .publishers
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<String>>()
                .join(", "),
            rights: &self.book.rights,
            issued: &self.book.issued,
            language: &self.book.language,
            isbn: &self.book.isbn,
            cover_image: &self.cover,
            authors: &self.book.authors,
            subjects: &self.book.subjects,
            styles: &self.stylesheets.values().collect(),
            chapters: &self.chapter_names,
            images: &images_mime
                .iter()
                .map(|(a, b)| (a.as_str(), b.as_str()))
                .collect(),
        };

        self.zip.write_file(
            OEBPS.as_path().join("content.opf"),
            content_opf
                .render()
                .context("failed to render content.opf")?
                .as_bytes(),
        )?;

        Ok(self)
    }

    fn parse_navpoints(
        elements: &[TocElement],
        mut order: usize,
        mut depth: usize,
    ) -> (usize, Vec<NavPoint>) {
        let navpoints = elements
            .iter()
            .map(|elem| {
                order += 1;
                let (child_depth, children) = Self::parse_navpoints(&elem.children, order, depth);
                depth = depth.max(elem.depth).max(child_depth);

                NavPoint {
                    id: if elem.fragment.is_empty() {
                        &elem.id
                    } else {
                        &elem.fragment
                    },
                    order,
                    children,
                    label: &elem.label,
                    url: &elem.href,
                }
            })
            .collect();

        (depth, navpoints)
    }

    // Render toc.ncx
    pub fn toc(&mut self, toc: &[TocElement]) -> Result<&mut Self> {
        let (depth, navpoints) = Self::parse_navpoints(toc, 0, 0);
        self.zip.write_file(
            OEBPS.as_path().join("toc.ncx"),
            Toc {
                uid: &self.book.isbn,
                depth,
                pagecount: self.book.pagecount,
                title: &self.book.title,
                author: &self
                    .book
                    .authors
                    .iter()
                    .map(|a| &a.name)
                    .cloned()
                    .collect::<Vec<String>>()
                    .join(", "),
                navpoints: &navpoints,
            }
            .render()
            .context("failed to render chapter xhtml")?
            .as_bytes(),
        )?;

        Ok(self)
    }
}
