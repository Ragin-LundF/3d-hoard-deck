use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid library-relative path: {0:?}")]
    InvalidPath(String),
    #[error("unknown code {code:?} for {what}")]
    UnknownCode { what: &'static str, code: String },
}
