use serde::Deserialize;

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
    full_path: String,
    url: String,
    original_url: String,
}

#[derive(Deserialize, Debug)]
pub struct ChapterNode {
    url: String,
    web_url: String,
    title: String,
}

#[derive(Deserialize, Debug)]
pub struct Chapter {
    title: String,
    filename: String,
    images: Vec<String>,
    stylesheets: Vec<Stylesheet>,
    site_styles: Vec<String>,
    content: String,
    next_chapter: ChapterNode,
    previous_chapter: ChapterNode,
}

#[derive(Deserialize, Debug)]
pub(crate) struct ChaptersResponse {
    pub count: usize,
    pub next: Option<String>,
    pub previous: Option<String>,
    pub results: Vec<Chapter>,
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
