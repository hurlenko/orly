use std::collections::HashMap;

use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};

use anyhow::Context;
use reqwest::{
    header::{
        HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, UPGRADE_INSECURE_REQUESTS, USER_AGENT,
    },
    Client, Url,
};
use serde::Deserialize;

use crate::{
    error::{OrlyError, Result},
    models::{BillingInfo, Book, Chapter, ChaptersResponse, Credentials, TocElement},
};

const CONCURRENT_REQUESTS: usize = 10;

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
}

impl<S: AuthState> OreillyClient<S> {
    fn make_url(&self, endpoint: &str) -> Result<Url> {
        Ok(self
            .base_url
            .join(endpoint)
            .with_context(|| format!("invalid endpoint: {}", endpoint))?)
    }
}

impl OreillyClient<Unauthenticated> {
    pub fn new() -> Self {
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
        }
    }

    async fn check_login(&self) -> Result<()> {
        let response = self
            .client
            .get(self.make_url("api/v1/subscriber/")?)
            .query(&[("format", "json")])
            .send()
            .await?;

        response.error_for_status_ref()?;

        #[derive(Deserialize, Debug)]
        struct Subscription {
            #[serde(rename = "Status")]
            status: String,
        }

        let subscription = response.json::<Subscription>().await?;

        if subscription.status != "Active" {
            return Err(OrlyError::SubscriptionExpired);
        }

        let response = self
            .client
            .get(self.make_url("api/v1/payments/next_billing_date/")?)
            .send()
            .await?;

        response.error_for_status_ref()?;

        let biling = response.json::<BillingInfo>().await?;

        let expiration = DateTime::parse_from_rfc3339(&biling.next_billing_date)
            .context("Failed to parse next billing date")?;

        println!("Subscription expiration: {}", expiration);

        if expiration < Utc::now() {
            return Err(OrlyError::SubscriptionExpired);
        }

        Ok(())
    }

    pub async fn cred_auth(
        self,
        email: String,
        password: String,
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

        match response.error_for_status_ref() {
            Err(err) => {
                return Err(OrlyError::AuthenticationFailed(format!(
                    "Login request failed, make sure your email and password are correct: {}",
                    err.to_string()
                )))
            }
            _ => (),
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
            marker: std::marker::PhantomData,
        })
    }
}

impl OreillyClient<Authenticated> {
    pub async fn fetch_book_deails(&self, book_id: String) -> Result<Book> {
        let response = self
            .client
            .get(self.make_url(&format!("api/v1/book/{}/", book_id))?)
            .send()
            .await?;

        response.error_for_status_ref()?;

        Ok(response.json::<Book>().await?)
    }

    pub async fn fetch_book_chapters(&self, book_id: String) -> Result<Vec<Chapter>> {
        println!("Loading chapter information");
        let url = self
            .make_url(&format!("api/v1/book/{}/chapter", book_id))?
            .to_string();

        let response = self
            .client
            .get(self.make_url(&format!("api/v1/book/{}/chapter", book_id))?)
            .send()
            .await?;

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

        let bodies = stream::iter(2..=pages)
            .map(|page| {
                let client = &self.client;
                let url = &url;

                async move {
                    let resp = client.get(url).query(&[("page", page)]).send().await?;
                    resp.json::<ChaptersResponse>().await
                }
            })
            .buffer_unordered(CONCURRENT_REQUESTS);

        let rest_pages = bodies
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;

        chapters.reserve_exact(total_chapters - per_page);
        chapters.extend(rest_pages.into_iter().flat_map(|r| r.results));

        Ok(chapters)
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
