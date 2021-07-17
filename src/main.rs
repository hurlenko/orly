pub mod model;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use thiserror::Error;

use anyhow::Context;
use reqwest::{
    header::{
        HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, UPGRADE_INSECURE_REQUESTS, USER_AGENT,
    },
    Client, Url,
};
use serde::Deserialize;

use crate::model::{BillingInfo, Book, Credentials};

#[derive(Error, Debug)]
pub enum OrlyError {
    #[error("Request failed")]
    HttpRequest(#[from] reqwest::Error),
    #[error("Authentication failure: {0}")]
    AuthenticationFailed(String),
    #[error("Subscription expired")]
    SubscriptionExpired,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    // source and Display delegate to anyhow::Error
    // #[error("invalid header (expected {expected:?}, found {found:?})")]
    // InvalidHeader { expected: String, found: String },
    // #[error("unknown data store error")]
    // Unknown,
}

type Result<T> = anyhow::Result<T, OrlyError>;

struct OreillyClient {
    client: Client,
    base_url: Url,
}

impl OreillyClient {
    fn new() -> Self {
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
        }
    }

    // async fn make_request(&self, method: String, url: String, endpoint: String) -> Response {

    // }

    async fn cred_auth(&self, email: String, password: String) -> Result<()> {
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

        // println!("{:#?}", response.bytes().await?);

        // let result: serde_json::Value = response.json().await?;

        let credentials = response.json::<Credentials>().await?;

        if !credentials.logged_in {
            return Err(OrlyError::AuthenticationFailed(
                "Expected to be logged in".to_string(),
            ));
        }

        // println!("{:#?}", credentials);

        Ok(())
    }

    fn make_url(&self, endpoint: &str) -> Result<Url> {
        Ok(self
            .base_url
            .join(endpoint)
            .with_context(|| format!("invalid endpoint: {}", endpoint))?)
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

        // "2021-07-23T20:51:36.072160Z"
        let expiration = DateTime::parse_from_rfc3339(&biling.next_billing_date)
            .context("Failed to parse next billing date")?;

        println!("Subscription expiration: {}", expiration);

        if expiration < Utc::now() {
            return Err(OrlyError::SubscriptionExpired);
        }

        Ok(())
    }

    async fn fetch_book_deails(&self, book_id: String) -> Result<Book> {
        let response = self
            .client
            .get(self.make_url(&format!("api/v1/book/{}/", book_id))?)
            .send()
            .await?;

        response.error_for_status_ref()?;

        Ok(response.json::<Book>().await?)
    }
}

async fn run() -> Result<()> {
    let book_id = "0735619670";
    let client = OreillyClient::new();
    client
        .cred_auth(
            "limowij820@godpeed.com".to_string(),
            "qwerty123".to_string(),
        )
        .await?;

    client.check_login().await?;

    let book = client.fetch_book_deails(book_id.to_string()).await?;
    println!("{:?}", book);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Err(err) = run().await {
        eprintln!("{:?}", err)
    }

    Ok(())
}
