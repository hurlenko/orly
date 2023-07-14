use std::collections::HashMap;

use bytes::Bytes;
use chrono::{DateTime, Local, NaiveDate};
use futures::stream::{self, StreamExt};

use anyhow::Context;
use log::{error, info, trace};
use reqwest::{
    header::{
        HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, COOKIE, UPGRADE_INSECURE_REQUESTS,
        USER_AGENT,
    },
    Client, ClientBuilder, Url,
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
        Self {
            client: Self::default_client().build().expect("to build the client"),
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

    fn default_client() -> ClientBuilder {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json,text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,image/apng,*/*;q=0.8"));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.212 Safari/537.36"));
        reqwest::Client::builder()
            .default_headers(headers)
            .cookie_store(true)
    }

    async fn check_subscription(&self, client: &Client) -> Result<()> {
        info!("Validating subscription");
        let response = client.get(self.make_url("api/v1/")?).send().await?;

        response.error_for_status_ref()?;

        let billing = response.json::<BillingInfo>().await?;

        trace!("Billing details: {:#?}", &billing);
        let expiration = if let Some(sub_exp) = billing.subscription.cancellation_date {
            let dt = NaiveDate::parse_from_str(&sub_exp, "%Y-%m-%d")
                .context("failed to parse subscription expiration ")?;
            dt.and_hms_opt(0, 0, 0).unwrap()
        } else if let Some(trial_exp) = billing.trial.trial_expiration_date {
            DateTime::parse_from_rfc3339(&trial_exp)
                .context("Failed to parse trial expiration date")?
                .naive_local()
        } else {
            return Err(crate::error::OrlyError::SubscriptionExpired);
        };

        info!("Subscription expiration: {}", expiration);

        if expiration < Local::now().naive_local() {
            error!("Subscription expired on {}", expiration);
            return Err(OrlyError::SubscriptionExpired);
        }

        Ok(())
    }

    pub async fn cred_auth(
        self,
        email: &str,
        password: &str,
    ) -> Result<OreillyClient<Authenticated>> {
        info!("Logging into Safari Books Online...");

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
                err
            )));
        }

        let credentials = response.json::<Credentials>().await?;

        if !credentials.logged_in {
            return Err(OrlyError::AuthenticationFailed(
                "Expected to be logged in".to_string(),
            ));
        }

        self.check_subscription(&self.client).await?;

        Ok(OreillyClient {
            client: self.client,
            base_url: self.base_url,
            concurrent_requests: self.concurrent_requests,
            marker: std::marker::PhantomData,
        })
    }

    pub async fn cookie_auth(self, cookie: &str) -> Result<OreillyClient<Authenticated>> {
        info!("Logging into Safari Books Online using cookies...");

        let mut request_headers = HeaderMap::new();
        request_headers.insert(
            COOKIE,
            HeaderValue::from_str(cookie).context("Invalid cookie")?,
        );

        let client = Self::default_client()
            .default_headers(request_headers)
            .build()?;
        self.check_subscription(&client).await?;

        Ok(OreillyClient {
            client,
            base_url: self.base_url,
            concurrent_requests: self.concurrent_requests,
            marker: std::marker::PhantomData,
        })
    }
}

impl OreillyClient<Authenticated> {
    pub async fn fetch_book_details(&self, book_id: &str) -> Result<Book> {
        info!("Fetching book details");
        let response = self
            .client
            .get(self.make_url(&format!("api/v1/book/{}/", book_id))?)
            .send()
            .await?;

        response.error_for_status_ref()?;
        let book = response.json::<Book>().await?;
        trace!("Book: {:#?}", &book);
        Ok(book)
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
        info!("Fetching chapter content");

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

        chapters.sort_by_key(|c| c.meta.position);

        trace!("Chapter content: {:?}", chapters);

        Ok(chapters)
    }

    async fn fetch_chapters_meta(&self, book_id: &str) -> Result<Vec<ChapterMeta>> {
        info!("Loading chapter information");
        let url = self
            .make_url(&format!("api/v1/book/{}/chapter", book_id))?
            .to_string();

        let response = self.client.get(url.clone()).send().await?;
        response.error_for_status_ref()?;

        let first_page = response.json::<ChaptersResponse>().await?;

        trace!("First page: {:#?}", first_page);

        let total_chapters = first_page.count;
        let per_page = first_page.results.len();
        let pages = (first_page.count + (per_page - 1)) / per_page;
        let mut chapters = first_page.results;

        info!(
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

        trace!("Chapters meta: {:?}", chapters);
        info!("Finished downloading chapter meta");

        Ok(chapters)
    }

    pub async fn fetch_book_chapters(&self, book_id: &str) -> Result<Vec<Chapter>> {
        let meta = self.fetch_chapters_meta(book_id).await?;
        self.fetch_chapters_content(meta).await
    }

    pub async fn fetch_toc(&self, book_id: &str) -> Result<Vec<TocElement>> {
        info!("Loading table of contents");

        let response = self
            .client
            .get(self.make_url(&format!("api/v1/book/{}/toc", book_id))?)
            .send()
            .await?;

        response.error_for_status_ref()?;

        let toc = response.json::<Vec<TocElement>>().await?;
        trace!("Table of contants: {:#?}", toc);
        Ok(toc)
    }
}
