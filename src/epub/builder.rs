use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::{OrlyError, Result};
use crate::models::Chapter;
use crate::templates::{BaseHtml, ContainerXml, IbooksXml};
use anyhow::Context;
use askama::Template;
use chrono;
use libxml::bindings::{
    xmlBufferContent, xmlBufferCreate, xmlBufferFree, xmlNodeDump, xmlSaveOption_XML_SAVE_AS_XML,
};
use libxml::parser::Parser;
use libxml::readonly::RoNode;
use libxml::tree::Document;
use libxml::xpath::{Context as XpathContext, Object};
use reqwest::Url;

use std::ffi::CStr;
use std::os::raw::c_char;

use super::zip::ZipArchive;
use lazy_static::lazy_static;

lazy_static! {
    static ref OEBPS: PathBuf = PathBuf::from("OEBPS");
}

#[derive(Debug)]
struct Metadata {
    pub title: String,
    pub author: String,
    pub lang: String,
    pub generator: String,
    pub toc_name: String,
    pub description: Option<String>,
    pub subject: Option<String>,
    pub license: Option<String>,
}

impl Metadata {
    pub fn new() -> Metadata {
        Metadata {
            title: String::new(),
            author: String::new(),
            lang: String::from("en"),
            generator: String::from("Rust EPUB library"),
            toc_name: String::from("Table Of Contents"),
            description: None,
            subject: None,
            license: None,
        }
    }
}

/// A file added in the EPUB
#[derive(Debug)]
struct Content {
    pub file: String,
    pub mime: String,
    pub itemref: bool,
    pub cover: bool,
    // pub reftype: Option<ReferenceType>,
    pub title: String,
}

impl Content {
    pub fn new<S1, S2>(file: S1, mime: S2) -> Content
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Content {
            file: file.into(),
            mime: mime.into(),
            itemref: false,
            cover: false,
            // reftype: None,
            title: String::new(),
        }
    }
}

pub struct EpubBuilder {
    zip: ZipArchive,
    // files: Vec<Content>,
    // metadata: Metadata,
    // toc: Toc,
    stylesheets: HashMap<Url, String>,
    images: HashSet<Url>,
    parser: Parser,
}

impl EpubBuilder {
    pub fn new() -> Result<Self> {
        let mut epub = EpubBuilder {
            zip: ZipArchive::new()?,
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

    pub fn ronode_to_string(&self, doc: &Document, node: &RoNode) -> String {
        unsafe {
            // allocate a buffer to dump into
            let buf = xmlBufferCreate();

            // dump the node
            xmlNodeDump(
                buf,
                doc.doc_ptr(),
                node.node_ptr(),
                1,                                    // level of indentation
                xmlSaveOption_XML_SAVE_AS_XML as i32, /* disable formatting */
            );
            let result = xmlBufferContent(buf);
            let c_string = CStr::from_ptr(result as *const c_char);
            let node_string = c_string.to_string_lossy().into_owned();
            xmlBufferFree(buf);

            node_string
        }
    }

    fn extract_chapter_content(&self, chapter_body: &String) -> Result<String> {
        let document = self.parser.parse_string(chapter_body)?;
        let context: XpathContext = XpathContext::new(&document)
            .map_err(OrlyError::XpathError)
            .context("Failed to create xpath context")?;
        let result: Object = context
            .evaluate("//div[@id='sbo-rt-content']")
            .map_err(OrlyError::XpathError)
            .context("Failed to evaluate xpath")?;

        let body = result.get_readonly_nodes_as_vec();
        assert_eq!(body.len(), 1);

        Ok(self.ronode_to_string(&document, &body[0]))
    }

    pub fn chapters(&mut self, chapters: Vec<Chapter>) -> Result<&mut Self> {
        for chapter in &chapters {
            let base_url = &chapter.meta.asset_base_url;
            self.images.extend(
                chapter
                    .meta
                    .images
                    .iter()
                    .map(|x| base_url.join(x))
                    .collect::<std::result::Result<Vec<Url>, _>>()
                    .context("Failed to join image url")?,
            );

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
                .with_extension("xhtml");

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

    // /// Render toc.ncx
    // fn render_toc(&mut self) -> Result<Vec<u8>> {
    //     let mut nav_points = String::new();

    //     nav_points.push_str(&self.toc.render_epub());

    //     let data = MapBuilder::new()
    //         .insert_str("toc_name", self.metadata.toc_name.as_str())
    //         .insert_str("nav_points", nav_points.as_str())
    //         .build();
    //     let mut res: Vec<u8> = vec![];
    //     templates::TOC_NCX
    //         .render_data(&mut res, &data)
    //         .chain_err(|| "error rendering toc.ncx template")?;
    //     Ok(res)
    // }

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
