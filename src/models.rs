use reqwest::Url;
use serde::{de::Error, Deserialize, Deserializer};
use std::result::Result as StdResult;

fn parse_url<'de, D>(deserializer: D) -> StdResult<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    Url::parse(s).map_err(D::Error::custom)
}

fn parse_vec_url<'de, D>(deserializer: D) -> StdResult<Vec<Url>, D::Error>
where
    D: Deserializer<'de>,
{
    let vec: Vec<&str> = Deserialize::deserialize(deserializer)?;
    vec.into_iter()
        .map(Url::parse)
        .collect::<std::result::Result<_, _>>()
        .map_err(D::Error::custom)
}

fn to_xhtml<'de, D>(deserializer: D) -> StdResult<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(s.replace(".html", ".xhtml"))
}

#[derive(Deserialize, Debug)]
pub(crate) struct BillingInfo {
    pub next_billing_date: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Credentials {
    pub logged_in: bool,
}

#[derive(Deserialize, Debug)]
pub struct Author {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct Subject {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct Publisher {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct Book {
    pub identifier: String,
    pub isbn: String,
    #[serde(deserialize_with = "parse_url")]
    pub cover: Url,
    pub chapter_list: String,
    pub toc: String,
    pub flat_toc: String,
    pub title: String,
    pub source: String,
    pub pagecount: usize,
    pub authors: Vec<Author>,
    pub subjects: Vec<Subject>,
    pub publishers: Vec<Publisher>,
    pub description: String,
    pub issued: String,
    #[serde(default)]
    pub rights: String,
    pub language: String,
}

#[derive(Deserialize, Debug)]
pub struct Stylesheet {
    pub full_path: String,
    #[serde(deserialize_with = "parse_url")]
    pub url: Url,
    pub original_url: String,
}

#[derive(Deserialize, Debug)]
pub struct ChapterNode {
    pub url: String,
    pub web_url: String,
    pub title: String,
}

#[derive(Deserialize, Debug)]
pub struct ChapterMeta {
    #[serde(deserialize_with = "parse_url")]
    pub asset_base_url: Url,
    pub title: String,
    #[serde(deserialize_with = "to_xhtml")]
    pub filename: String,
    pub images: Vec<String>,
    pub stylesheets: Vec<Stylesheet>,
    #[serde(deserialize_with = "parse_vec_url")]
    pub site_styles: Vec<Url>,
    #[serde(deserialize_with = "parse_url", rename = "content")]
    pub content_url: Url,
    #[serde(default)]
    pub position: usize,
}

#[derive(Debug)]
pub struct Chapter {
    pub meta: ChapterMeta,
    pub content: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChaptersResponse {
    pub count: usize,
    pub results: Vec<ChapterMeta>,
}

#[derive(Deserialize, Debug)]
pub struct TocElement {
    pub depth: usize,
    pub url: String,
    pub minutes_required: f64,
    pub fragment: String,
    pub filename: String,
    pub natural_key: Vec<String>,
    pub label: String,
    pub full_path: String,
    #[serde(deserialize_with = "to_xhtml")]
    pub href: String,
    pub id: String,
    pub media_type: String,
    pub children: Vec<TocElement>,
}
