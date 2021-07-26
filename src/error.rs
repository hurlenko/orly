use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrlyError {
    #[error("Request failed: {0}")]
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

pub type Result<T> = anyhow::Result<T, OrlyError>;
