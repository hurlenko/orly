use reqwest::Url;
use serde::{de::Error, Deserialize, Deserializer};

fn parse_url<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    Url::parse(s).map_err(D::Error::custom)
}

fn parse_vec_url<'de, D>(deserializer: D) -> Result<Vec<Url>, D::Error>
where
    D: Deserializer<'de>,
{
    let vec: Vec<&str> = Deserialize::deserialize(deserializer)?;
    vec.into_iter()
        .map(|s| Url::parse(s))
        .collect::<std::result::Result<_, _>>()
        .map_err(D::Error::custom)
}

#[derive(Deserialize, Debug)]
pub(crate) struct BillingInfo {
    pub next_billing_date: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Credentials {
    id_token: String,
    refresh_token: String,
    pub logged_in: bool,
    expires_at: String,
    redirect_uri: String,
    uuid: String,
}

//

#[derive(Deserialize, Debug)]
pub struct Book {
    // chapters: Vec<String>,
    pub cover: String,
    pub chapter_list: String,
    pub toc: String,
    pub flat_toc: String,
    pub title: String,
    pub source: String,
    pub pagecount: usize,
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
    pub filename: String,
    pub images: Vec<String>,
    pub stylesheets: Vec<Stylesheet>,
    #[serde(deserialize_with = "parse_vec_url")]
    pub site_styles: Vec<Url>,
    #[serde(deserialize_with = "parse_url", rename = "content")]
    pub content_url: Url,
    // pub next_chapter: ChapterNode,
    // pub previous_chapter: ChapterNode,
}


#[derive(Deserialize, Debug)]
pub struct Chapter {
    pub meta: ChapterMeta,
    pub content: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChaptersResponse {
    pub count: usize,
    pub next: Option<String>,
    pub previous: Option<String>,
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
    pub href: String,
    pub id: String,
    pub media_type: String,
    pub children: Vec<Box<TocElement>>,
}
