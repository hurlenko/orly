use std::collections::HashMap;

use bytes::Bytes;
use chrono::{DateTime, Local, Utc};
use futures::stream::{self, StreamExt};

use anyhow::Context;
use reqwest::{
    header::{
        HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, UPGRADE_INSECURE_REQUESTS, USER_AGENT,
    },
    Client, Url,
};

use crate::{
    error::{OrlyError, Result},
    models::{BillingInfo, Book, Chapter, ChapterMeta, ChaptersResponse, Credentials, TocElement},
};

pub struct Authenticated;
pub struct Unauthenticated;
mod private {
    pub trait Sealed {}

    impl Sealed for super::Authenticated {}
    impl Sealed for super::Unauthenticated {}
}

pub trait AuthState: private::Sealed {}
impl AuthState for Authenticated {}
impl AuthState for Unauthenticated {}

pub struct OreillyClient<S: AuthState> {
    client: Client,
    base_url: Url,
    marker: std::marker::PhantomData<S>,
    concurrent_requests: usize,
}

impl<S: AuthState> OreillyClient<S> {
    fn make_url(&self, endpoint: &str) -> Result<Url> {
        Ok(self
            .base_url
            .join(endpoint)
            .with_context(|| format!("invalid endpoint: {}", endpoint))?)
    }
}

impl Default for OreillyClient<Unauthenticated> {
    fn default() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json,text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,image/apng,*/*;q=0.8"));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.212 Safari/537.36"));
        Self {
            client: reqwest::Client::builder()
                .default_headers(headers)
                .cookie_store(true)
                .build()
                .expect("to build the client"),
            base_url: "https://learning.oreilly.com"
                .parse()
                .expect("correct base url"),
            marker: std::marker::PhantomData,
            concurrent_requests: 20,
        }
    }
}

impl OreillyClient<Unauthenticated> {
    pub fn new(concurrent_requests: usize) -> Self {
        Self {
            concurrent_requests,
            ..Default::default()
        }
    }

    async fn check_login(&self) -> Result<()> {
        println!("Validating subscription");
        let response = self
            .client
            .get(self.make_url("api/v1/payments/next_billing_date/")?)
            .send()
            .await?;

        response.error_for_status_ref()?;

        let billing = response.json::<BillingInfo>().await?;

        let expiration = DateTime::parse_from_rfc3339(&billing.next_billing_date)
            .context("Failed to parse next billing date")?;

        let local: DateTime<Local> = DateTime::from(expiration);

        println!("Subscription expiration: {}", local);

        if expiration < Utc::now() {
            return Err(OrlyError::SubscriptionExpired);
        }

        Ok(())
    }

    pub async fn cred_auth(
        self,
        email: &str,
        password: &str,
    ) -> Result<OreillyClient<Authenticated>> {
        println!("Logging into Safari Books Online...");

        let mut map = HashMap::new();
        map.insert("email", email);
        map.insert("password", password);

        let response = self
            .client
            .post("https://www.oreilly.com/member/auth/login/")
            .json(&map)
            .send()
            .await?;

        if let Err(err) = response.error_for_status_ref() {
            return Err(OrlyError::AuthenticationFailed(format!(
                "Login request failed, make sure your email and password are correct: {}",
                err.to_string()
            )));
        }

        let credentials = response.json::<Credentials>().await?;

        if !credentials.logged_in {
            return Err(OrlyError::AuthenticationFailed(
                "Expected to be logged in".to_string(),
            ));
        }

        self.check_login().await?;

        Ok(OreillyClient {
            client: self.client,
            base_url: self.base_url,
            concurrent_requests: self.concurrent_requests,
            marker: std::marker::PhantomData,
        })
    }
}

impl OreillyClient<Authenticated> {
    pub async fn fetch_book_details(&self, book_id: &str) -> Result<Book> {
        let response = self
            .client
            .get(self.make_url(&format!("api/v1/book/{}/", book_id))?)
            .send()
            .await?;

        response.error_for_status_ref()?;

        Ok(response.json::<Book>().await?)
    }

    pub async fn bulk_download_bytes<'a, T: IntoIterator<Item = &'a Url>>(
        &'a self,
        urls: T,
    ) -> Result<Vec<(&'a Url, Bytes)>> {
        let responses = stream::iter(urls.into_iter())
            .map(|url| async move {
                let resp = self.client.get(url.clone()).send().await?.bytes().await?;
                Ok::<(&'a Url, Bytes), OrlyError>((url, resp))
            })
            .buffer_unordered(self.concurrent_requests);

        let responses = responses
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(responses)
    }

    pub async fn download_text(&self, url: Url) -> Result<String> {
        Ok(self.client.get(url).send().await?.text().await?)
    }

    async fn fetch_chapters_content(
        &self,
        chapters_meta: Vec<ChapterMeta>,
    ) -> Result<Vec<Chapter>> {
        println!("Fetching chapter content");

        let chapters = stream::iter(chapters_meta.into_iter())
            .map(|meta| async move {
                let content = self.download_text(meta.content_url.clone()).await?;
                Ok::<Chapter, OrlyError>(Chapter { meta, content })
            })
            .buffer_unordered(self.concurrent_requests);

        let mut chapters = chapters
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;

        println!("#chapters: {}", chapters.len());

        chapters.sort_by_key(|c| c.meta.position);

        Ok(chapters)
    }

    async fn fetch_chapters_meta(&self, book_id: &str) -> Result<Vec<ChapterMeta>> {
        println!("Loading chapter information");
        let url = self
            .make_url(&format!("api/v1/book/{}/chapter", book_id))?
            .to_string();

        let response = self.client.get(url.clone()).send().await?;
        response.error_for_status_ref()?;

        let first_page = response.json::<ChaptersResponse>().await?;

        let total_chapters = first_page.count;
        let per_page = first_page.results.len();
        let pages = (first_page.count + (per_page - 1)) / per_page;
        let mut chapters = first_page.results;

        println!(
            "Downloading {} chapters, {} chapters per page, {} pages",
            total_chapters, per_page, pages
        );

        let pages = stream::iter(2..=pages)
            .map(|page| {
                let client = &self.client;
                let url = &url;

                async move {
                    let resp = client.get(url).query(&[("page", page)]).send().await?;
                    resp.json::<ChaptersResponse>().await
                }
            })
            .buffered(self.concurrent_requests);

        let rest_pages = pages
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;

        chapters.reserve_exact(total_chapters - per_page);
        chapters.extend(rest_pages.into_iter().flat_map(|r| r.results));

        for (position, chapter) in chapters.iter_mut().enumerate() {
            chapter.position = position;
        }

        println!("Finished downloading chapter meta");

        Ok(chapters)
    }

    pub async fn fetch_book_chapters(&self, book_id: &str) -> Result<Vec<Chapter>> {
        let meta = self.fetch_chapters_meta(book_id).await?;
        println!("#meta: {}", meta.len());
        self.fetch_chapters_content(meta).await
    }

    pub async fn fetch_toc(&self, book_id: &str) -> Result<Vec<TocElement>> {
        println!("Loading table of contents");

        let response = self
            .client
            .get(self.make_url(&format!("api/v1/book/{}/toc", book_id))?)
            .send()
            .await?;

        response.error_for_status_ref()?;

        Ok(response.json::<Vec<TocElement>>().await?)
    }
}
