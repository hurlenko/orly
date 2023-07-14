use std::collections::{HashMap, HashSet};

use crate::{
    client::{Authenticated, OreillyClient},
    epub::lxml::DocumentExt,
    error::{OrlyError, Result},
    models::{Book, Chapter, TocElement},
    templates::{ChapterXhtml, ContainerXml, ContentOpf, IbooksXml, NavPoint, Toc},
};
use std::{
    ffi::OsStr,
    io::Cursor,
    path::{Path, PathBuf},
};

use anyhow::Context;
use askama::Template;

use image::{imageops::FilterType, ImageFormat, ImageOutputFormat};
use libxml::{parser::Parser, tree::SaveOptions};
use lightningcss::{
    declaration::DeclarationBlock,
    dependencies::{Dependency, DependencyOptions},
    properties::{
        display::{Display, DisplayKeyword, Visibility},
        Property,
    },
    rules::{style::StyleRule, CssRule, CssRuleList},
};
use log::{debug, info, warn};
use reqwest::Url;
use url::ParseError;

use super::zip::ZipArchive;
use lazy_static::lazy_static;

use bytes::Bytes;
use image::io::Reader as ImageReader;
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};

const XHTML: &str = "xhtml";
const IMAGES: &str = "Images";
const STYLES: &str = "Styles";
const TEXT: &str = "Text";

lazy_static! {
    static ref OEBPS: PathBuf = PathBuf::from("OEBPS");
}

pub struct EpubBuilder<'a> {
    zip: ZipArchive,
    book: &'a Book,
    base_files_url: Url,
    stylesheets: HashMap<Url, String>,
    images: HashMap<Url, String>,
    parser: Parser,
    chapter_names: Vec<String>,
    // image name
    cover: String,
    kindle: bool,
}

impl<'a> EpubBuilder<'a> {
    pub fn new(book: &'a Book, kindle: bool) -> Result<Self> {
        let mut epub = EpubBuilder {
            zip: ZipArchive::new()?,
            book,
            base_files_url: Url::parse(&format!(
                "https://learning.oreilly.com/api/v2/epubs/urn:orm:book:{}/files/",
                book.identifier
            ))
            .unwrap(),
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
            Some("html") => path.with_extension(XHTML).to_str().map(str::to_string),
            Some(ext) if ImageFormat::from_extension(ext).is_some() => path
                .to_str()
                .map(|filename| format!("../{}/{}", IMAGES, filename)),
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
        // let stripped = document.strip_invalid_attributes();
        // warn!("Invalid attributes stripped: {}", stripped);

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
        let image_urls = chapter
            .meta
            .images
            .iter()
            .map(|x| {
                self.base_files_url.join(x).ok().and_then(|url| {
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

        let filename = format!("{}/{}", TEXT, chapter.meta.filename);

        self.zip.write_file(
            OEBPS.as_path().join(&filename),
            chapter_xhtml
                .render()
                .context("failed to render chapter xhtml")?
                .as_bytes(),
        )?;
        self.chapter_names.push(filename);

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

            self.add_chapter(chapter)?;
        }

        info!("Found {} images", self.images.len());
        info!("Found {} stylesheets", self.stylesheets.len());
        Ok(self)
    }

    fn rewrite_css_rules(parent_rules: &mut CssRuleList) {
        // As of 2022 Send To Kindle supports epub natively. However files are still being
        // converted internally (to mobi/azw3 ??). During this conversion process, the epub
        // gets validated according to
        // Kindle Publishing Guidelines https://kdp.amazon.com/en_US/help/topic/GR4KL488MXKPZ5BK,
        // which has a rule:
        // "Kindle limits usage of the display:none property for content blocks
        // beyond 10000 characters. If the display:none property is applied to a content block
        // that is bigger than 10000 characters, Kindle Previewer returns an error."
        // However, the conversion tool (kindlegen ??) does not handle complex css rules
        // properly (https://github.com/dvschultz/99problems/issues/50) which causes epubs sent
        // via Send To Kindle to be rejected.
        // The best workaround I came up with is to replace all "display: none" with
        // "visibility: hidden". It's not the same as it leaves empty space but it's pretty close.
        for rule in parent_rules.0.iter_mut() {
            if let CssRule::Style(StyleRule {
                declarations:
                    DeclarationBlock {
                        declarations,
                        important_declarations,
                    },
                rules,
                ..
            }) = rule
            {
                for property in declarations
                    .iter_mut()
                    .chain(important_declarations.iter_mut())
                {
                    if let Property::Display(Display::Keyword(DisplayKeyword::None)) = property {
                        warn!("Found display: none, replacing");
                        *property = Property::Visibility(Visibility::Hidden)
                    }
                }
                if !rules.0.is_empty() {
                    Self::rewrite_css_rules(rules)
                }
            };
        }
    }

    fn optimize_image(&self, source_bytes: Bytes) -> (ImageFormat, Bytes) {
        const KINDLE_WIDTH: u32 = 1072;

        let image_reader = ImageReader::new(Cursor::new(&source_bytes));
        // Skip everything smaller than this
        if source_bytes.len() < 60 * 1024 {
            debug!(
                "File is too small ({}), skipping optimizations",
                source_bytes.len()
            );
            return (
                image_reader.format().unwrap_or(ImageFormat::Jpeg),
                source_bytes,
            );
        }
        let mut source_image = image_reader
            .with_guessed_format()
            .expect("Unknown image format")
            .decode()
            .expect("Unknown image format");

        if self.kindle && source_image.width() > KINDLE_WIDTH {
            debug!(
                "Image is too big {}x{}, resing",
                source_image.width(),
                source_image.height()
            );
            source_image =
                source_image.resize(KINDLE_WIDTH, source_image.height(), FilterType::Lanczos3);
            debug!(
                "New size: {}x{}",
                source_image.width(),
                source_image.height()
            );
        }

        let mut result = Cursor::new(Vec::new());
        let mut format = ImageFormat::Jpeg;

        if source_image.color().has_alpha() {
            debug!("Image has alpha channel, saving as png");
            format = ImageFormat::Png;
        }

        source_image
            .write_to(&mut result, ImageOutputFormat::from(format))
            .expect("Failed to encode image");

        let optimized = Bytes::copy_from_slice(result.get_ref());
        debug!(
            "Old image size: {}, new size: {}, relative change: {:.2}%",
            source_bytes.len(),
            optimized.len(),
            (optimized.len() as f32 - source_bytes.len() as f32) / optimized.len() as f32 * 100.0
        );

        (format, optimized)
    }

    pub async fn generate<W: tokio::io::AsyncWrite + std::marker::Unpin>(
        &mut self,
        to: W,
        client: &OreillyClient<Authenticated>,
    ) -> Result<()> {
        // Unique urls != unique filenames
        let images_count = self.images.len();
        let unique_images = self.images.values().collect::<HashSet<&String>>().len();
        if images_count != unique_images {
            warn!("Images have non-unique names, some of them might get overwritten");
        }

        info!("Downloading and optimizing {} images", self.images.len());
        let mut image_mimetypes: Vec<(String, String)> = Vec::with_capacity(self.images.len());
        for (url, bytes) in client.bulk_download_bytes(self.images.keys()).await? {
            debug!("Optimizing image {}", url);
            let (extension, bytes) = self.optimize_image(bytes);
            let filename = self.images.get(url).unwrap().clone();

            self.zip
                .write_file(OEBPS.as_path().join(&filename), &*bytes)?;

            image_mimetypes.push((filename, format!("{:?}", extension).to_ascii_lowercase()));
        }

        info!("Downloading {} css", self.stylesheets.len());
        let mut css_dependencies = HashMap::new();
        for (url, bytes) in client.bulk_download_bytes(self.stylesheets.keys()).await? {
            let mut stylesheet = StyleSheet::parse(
                std::str::from_utf8(&bytes[..]).unwrap(),
                ParserOptions::default(),
            )
            .unwrap();

            if self.kindle {
                Self::rewrite_css_rules(&mut stylesheet.rules);
            }
            stylesheet.minify(MinifyOptions::default()).unwrap();
            let deps = stylesheet
                .to_css(PrinterOptions {
                    analyze_dependencies: Some(DependencyOptions {
                        remove_imports: true,
                    }),
                    ..PrinterOptions::default()
                })
                .unwrap()
                .dependencies;

            for dependency in deps.unwrap_or_default() {
                match dependency {
                    Dependency::Url(url) => {
                        css_dependencies.insert(
                            self.base_files_url
                                .join(&url.url)
                                .expect("Failed to build css deps url"),
                            format!("{}/{}", STYLES, url.url),
                        );
                    }
                    Dependency::Import(import) => warn!("css import dependecy: {:?}", import.url),
                }
            }

            let res = stylesheet
                .to_css(PrinterOptions {
                    minify: true,
                    ..PrinterOptions::default()
                })
                .expect("Failed to convert to css");

            self.zip.write_file(
                OEBPS.as_path().join(self.stylesheets.get(url).unwrap()),
                res.code.as_bytes(),
            )?;
        }

        info!("Downloading {} css dependencies", css_dependencies.len());
        for (url, bytes) in client.bulk_download_bytes(css_dependencies.keys()).await? {
            self.zip.write_file(
                OEBPS.as_path().join(css_dependencies.get(url).unwrap()),
                &bytes[..],
            )?;
        }

        info!("Rendering OPF and generating final EPUB");
        self.render_opf(&image_mimetypes, &css_dependencies.values().collect())?
            .zip
            .generate(to)
            .await?;
        Ok(())
    }

    /// Render content.opf file
    fn render_opf(
        &mut self,
        image_mimetypes: &Vec<(String, String)>,
        css_deps: &Vec<&String>,
    ) -> Result<&mut Self> {
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
            images: image_mimetypes,
            css_deps,
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
    ) -> (usize, usize, Vec<NavPoint>) {
        let navpoints = elements
            .iter()
            .map(|elem| {
                let (child_depth, new_order, children) =
                    Self::parse_navpoints(&elem.children, order + 1, depth);
                depth = depth.max(elem.depth).max(child_depth);

                let navpoint = NavPoint {
                    id: if elem.fragment.is_empty() {
                        &elem.id
                    } else {
                        &elem.fragment
                    },
                    order,
                    children,
                    label: &elem.label,
                    url: format!("{}/{}", TEXT, elem.href),
                };
                order = new_order;
                navpoint
            })
            .collect();

        (depth, order, navpoints)
    }

    // Render toc.ncx
    pub fn toc(&mut self, toc: &[TocElement]) -> Result<&mut Self> {
        let (depth, _, navpoints) = Self::parse_navpoints(toc, 0, 0);
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
