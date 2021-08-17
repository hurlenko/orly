use std::collections::{HashMap, HashSet};

use std::{ffi::OsStr, path::PathBuf};

use crate::{
    epub::lxml::DocumentExt,
    error::Result,
    models::{Book, Chapter, TocElement},
    templates::{BaseHtml, ContainerXml, IbooksXml, NavPoint, Toc},
};

use anyhow::Context;
use askama::Template;

use libxml::{parser::Parser, tree::SaveOptions};
use reqwest::Url;
use url::ParseError;

use super::zip::ZipArchive;
use lazy_static::lazy_static;

const XHTML: &str = "xhtml";

lazy_static! {
    static ref OEBPS: PathBuf = PathBuf::from("OEBPS");
}

pub struct EpubBuilder<'a> {
    zip: ZipArchive,
    book: &'a Book,
    // files: Vec<Content>,
    // metadata: Metadata,
    // toc: Toc,
    // Absolute url, final filename
    stylesheets: HashMap<Url, String>,
    // Relative url from API, absolute url with asset_base_url, final filename
    images: HashMap<Url, String>,
    parser: Parser,
}

impl<'a> EpubBuilder<'a> {
    pub fn new(book: &'a Book) -> Result<Self> {
        let mut epub = EpubBuilder {
            zip: ZipArchive::new()?,
            book,
            stylesheets: Default::default(),
            images: Default::default(),
            parser: Parser::default_html(),
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
            Some("png" | "jpg" | "jpeg" | "gif") => {
                path.to_str().map(|filename| format!("images/{}", filename))
            }
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
        document.rewrite_links(|old| self.rewrite_chapter_links(old));
        // for (node, attrs) in document.iterlinks() {
        //     if attrs.len() > 0 {
        //         println!(
        //             "{:?} - {}",
        //             attrs,
        //             document.node_to_string_with_options(
        //                 &node,
        //                 SaveOptions {
        //                     as_xml: true,
        //                     ..Default::default()
        //                 }
        //             )
        //         );
        //     }
        // }

        let body = document.xpath("//div[@id='sbo-rt-content']");
        assert_eq!(body.len(), 1);

        Ok(document.node_to_string_with_options(
            &body[0],
            SaveOptions {
                as_xml: true,
                ..Default::default()
            },
        ))
    }

    pub fn chapters(&mut self, chapters: &[Chapter]) -> Result<&mut Self> {
        for chapter in chapters {
            let base_url = &chapter.meta.asset_base_url;
            let image_urls = chapter
                .meta
                .images
                .iter()
                .map(|x| {
                    base_url.join(x).ok().and_then(|url| {
                        PathBuf::from(url.path())
                            .file_name()
                            .and_then(OsStr::to_str)
                            .map(|filename| (url, filename.to_string()))
                    })
                })
                .collect::<Option<Vec<_>>>()
                .context("Failed to join image url")?;
            self.images.extend(image_urls);

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
                    .or_insert(format!("{}.css", count));
            }

            let chapter_xhtml = BaseHtml {
                styles: &self.stylesheets.values().collect(),
                body: &self.extract_chapter_content(&chapter.content)?,
                should_support_kindle: true,
            };

            let filename = OEBPS
                .as_path()
                .join(&chapter.meta.filename)
                .with_extension(XHTML);

            self.zip.write_file(
                filename,
                chapter_xhtml
                    .render()
                    .context("failed to render chapter xhtml")?
                    .as_bytes(),
            )?;
        }

        println!("Found {} images", self.images.len());
        println!("Found {} stylesheets", self.stylesheets.len());
        Ok(self)
    }

    // pub fn metadata<S1, S2>(&mut self, key: S1, value: S2) -> &mut Self
    // where
    //     S1: AsRef<str>,
    //     S2: Into<String>,
    // {
    //     match key.as_ref() {
    //         "author" => self.metadata.author = value.into(),
    //         "title" => self.metadata.title = value.into(),
    //         "lang" => self.metadata.lang = value.into(),
    //         "generator" => self.metadata.generator = value.into(),
    //         "description" => self.metadata.description = Some(value.into()),
    //         "subject" => self.metadata.subject = Some(value.into()),
    //         "license" => self.metadata.license = Some(value.into()),
    //         "toc_name" => self.metadata.toc_name = value.into(),
    //         s => unreachable!("invalid metadata '{}'", s),
    //     }
    //     self
    // }

    // pub fn stylesheet<R: Read>(&mut self, content: R) -> Result<&mut Self> {
    //     self.add_resource("stylesheet.css", content, "text/css")?;
    //     self.stylesheet = true;
    //     Ok(self)
    // }

    // pub fn inline_toc(&mut self) -> &mut Self {
    //     self.inline_toc = true;
    //     self.toc.add(TocElement::new(
    //         "toc.xhtml",
    //         self.metadata.toc_name.as_str(),
    //     ));
    //     let mut file = Content::new("toc.xhtml", "application/xhtml+xml");
    //     file.reftype = Some(ReferenceType::Toc);
    //     file.title = self.metadata.toc_name.clone();
    //     file.itemref = true;
    //     self.files.push(file);
    //     self
    // }

    // pub fn add_cover_image<R, P, S>(
    //     &mut self,
    //     path: P,
    //     content: R,
    //     mime_type: S,
    // ) -> Result<&mut Self>
    // where
    //     R: Read,
    //     P: AsRef<Path>,
    //     S: Into<String>,
    // {
    //     self.zip
    //         .write_file(Path::new("OEBPS").join(path.as_ref()), content)?;
    //     let mut file = Content::new(format!("{}", path.as_ref().display()), mime_type);
    //     file.cover = true;
    //     self.files.push(file);
    //     Ok(self)
    // }

    // pub fn add_content<R: Read>(&mut self, content: EpubContent<R>) -> Result<&mut Self> {
    //     self.zip.write_file(
    //         Path::new("OEBPS").join(content.toc.url.as_str()),
    //         content.content,
    //     )?;
    //     let mut file = Content::new(content.toc.url.as_str(), "application/xhtml+xml");
    //     file.itemref = true;
    //     file.reftype = content.reftype;
    //     if file.reftype.is_some() {
    //         file.title = content.toc.title.clone();
    //     }
    //     self.files.push(file);
    //     if !content.toc.title.is_empty() {
    //         self.toc.add(content.toc);
    //     }
    //     Ok(self)
    // }

    pub async fn generate<W: tokio::io::AsyncWrite + std::marker::Unpin>(
        &mut self,
        to: W,
    ) -> Result<()> {
        // If no styleesheet was provided, generate a dummy one
        // if !self.stylesheet {
        //     self.stylesheet(b"".as_ref())?;
        // }
        // // Render content.opf
        // let bytes = self.render_opf()?;
        // self.zip.write_file("OEBPS/content.opf", &*bytes)?;
        // // Render toc.ncx
        // let bytes = self.render_toc()?;
        // self.zip.write_file("OEBPS/toc.ncx", &*bytes)?;
        // // Render nav.xhtml
        // let bytes = self.render_nav(true)?;
        // self.zip.write_file("OEBPS/nav.xhtml", &*bytes)?;
        // // Write inline toc if it needs to
        // if self.inline_toc {
        //     let bytes = self.render_nav(false)?;
        //     self.zip.write_file("OEBPS/toc.xhtml", &*bytes)?;
        // }
        // Unique urls != unique filenames
        let images_count = self.images.len();
        let unique_images = self.images.values().collect::<HashSet<&String>>().len();
        assert_eq!(images_count, unique_images);

        self.zip.generate(to).await?;
        Ok(())
    }

    // /// Render content.opf file
    // fn render_opf(&mut self) -> Result<Vec<u8>> {
    //     let mut optional = String::new();
    //     if let Some(ref desc) = self.metadata.description {
    //         write!(optional, "<dc:description>{}</dc:description>\n", desc)?;
    //     }
    //     if let Some(ref subject) = self.metadata.subject {
    //         write!(optional, "<dc:subject>{}</dc:subject>\n", subject)?;
    //     }
    //     if let Some(ref rights) = self.metadata.license {
    //         write!(optional, "<dc:rights>{}</dc:rights>\n", rights)?;
    //     }
    //     let date = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    //     let uuid = uuid::adapter::Urn::from_uuid(uuid::Uuid::new_v4()).to_string();

    //     let mut items = String::new();
    //     let mut itemrefs = String::new();
    //     let mut guide = String::new();

    //     for content in &self.files {
    //         let id = if content.cover {
    //             String::from("cover-image")
    //         } else {
    //             to_id(&content.file)
    //         };
    //         let properties = match (self.version, content.cover) {
    //             (EpubVersion::V30, true) => "properties=\"cover-image\"",
    //             _ => "",
    //         };
    //         if content.cover {
    //             write!(
    //                 optional,
    //                 "<meta name=\"cover\" content=\"cover-image\" />\n"
    //             )?;
    //         }
    //         write!(
    //             items,
    //             "<item media-type=\"{mime}\" {properties} \
    //                 id=\"{id}\" href=\"{href}\" />\n",
    //             properties = properties,
    //             mime = content.mime,
    //             id = id,
    //             href = content.file
    //         )?;
    //         if content.itemref {
    //             write!(itemrefs, "<itemref idref=\"{id}\" />\n", id = id)?;
    //         }
    //         if let Some(reftype) = content.reftype {
    //             use epub_content::ReferenceType::*;
    //             let reftype = match reftype {
    //                 Cover => "cover",
    //                 TitlePage => "title-page",
    //                 Toc => "toc",
    //                 Index => "index",
    //                 Glossary => "glossary",
    //                 Acknowledgements => "acknowledgements",
    //                 Bibliography => "bibliography",
    //                 Colophon => "colophon",
    //                 Copyright => "copyright",
    //                 Dedication => "dedication",
    //                 Epigraph => "epigraph",
    //                 Foreword => "foreword",
    //                 Loi => "loi",
    //                 Lot => "lot",
    //                 Notes => "notes",
    //                 Preface => "preface",
    //                 Text => "text",
    //             };
    //             write!(
    //                 guide,
    //                 "<reference type=\"{reftype}\" title=\"{title}\" href=\"{href}\" />\n",
    //                 reftype = reftype,
    //                 title = common::escape_quote(content.title.as_str()),
    //                 href = content.file
    //             )?;
    //         }
    //     }

    //     let data = MapBuilder::new()
    //         .insert_str("lang", self.metadata.lang.as_str())
    //         .insert_str("author", self.metadata.author.as_str())
    //         .insert_str("title", self.metadata.title.as_str())
    //         .insert_str("generator", self.metadata.generator.as_str())
    //         .insert_str("toc_name", self.metadata.toc_name.as_str())
    //         .insert_str("optional", optional)
    //         .insert_str("items", items)
    //         .insert_str("itemrefs", itemrefs)
    //         .insert_str("date", date.to_string())
    //         .insert_str("uuid", uuid)
    //         .insert_str("guide", guide)
    //         .build();

    //     let mut content = vec![];
    //     let res = match self.version {
    //         EpubVersion::V20 => templates::v2::CONTENT_OPF.render_data(&mut content, &data),
    //         EpubVersion::V30 => templates::v3::CONTENT_OPF.render_data(&mut content, &data),
    //         EpubVersion::__NonExhaustive => unreachable!(),
    //     };

    //     res.chain_err(|| "could not render template for content.opf")?;

    //     Ok(content)
    // }

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
                    url: elem.href.replace(".html", &format!(".{}", XHTML)),
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

    // /// Render nav.xhtml
    // fn render_nav(&mut self, numbered: bool) -> Result<Vec<u8>> {
    //     let content = self.toc.render(numbered);
    //     let mut landmarks = String::new();
    //     if self.version > EpubVersion::V20 {
    //         for file in &self.files {
    //             if let Some(ref reftype) = file.reftype {
    //                 use ReferenceType::*;
    //                 let reftype = match *reftype {
    //                     Cover => "cover",
    //                     Text => "bodymatter",
    //                     Toc => "toc",
    //                     Bibliography => "bibliography",
    //                     Epigraph => "epigraph",
    //                     Foreword => "foreword",
    //                     Preface => "preface",
    //                     Notes => "endnotes",
    //                     Loi => "loi",
    //                     Lot => "lot",
    //                     Colophon => "colophon",
    //                     TitlePage => "titlepage",
    //                     Index => "index",
    //                     Glossary => "glossary",
    //                     Copyright => "copyright-page",
    //                     Acknowledgements => "acknowledgements",
    //                     Dedication => "dedication",
    //                 };
    //                 if !file.title.is_empty() {
    //                     write!(
    //                         landmarks,
    //                         "<li><a epub:type=\"{reftype}\" href=\"{href}\">\
    //                             {title}</a></li>\n",
    //                         reftype = reftype,
    //                         href = file.file,
    //                         title = file.title
    //                     )?;
    //                 }
    //             }
    //         }
    //     }
    //     if !landmarks.is_empty() {
    //         landmarks = format!("<ol>\n{}\n</ol>", landmarks);
    //     }

    //     let data = MapBuilder::new()
    //         .insert_str("content", content)
    //         .insert_str("toc_name", self.metadata.toc_name.as_str())
    //         .insert_str("generator", self.metadata.generator.as_str())
    //         .insert_str("landmarks", landmarks)
    //         .build();

    //     let mut res = vec![];
    //     let eh = match self.version {
    //         EpubVersion::V20 => templates::v2::NAV_XHTML.render_data(&mut res, &data),
    //         EpubVersion::V30 => templates::v3::NAV_XHTML.render_data(&mut res, &data),
    //         EpubVersion::__NonExhaustive => unreachable!(),
    //     };

    //     eh.chain_err(|| "error rendering nav.xhtml template")?;
    //     Ok(res)
    // }
}
