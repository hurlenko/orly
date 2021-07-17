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
    pagecount: u32,
}
