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
    cover: String,
    chapter_list: String,
    toc: String,
    flat_toc: String,
    title: String,
    source: String,
    pagecount: usize,
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
