use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid library-relative path: {0:?}")]
    InvalidPath(String),
    #[error("metadata files beside the models need write access to the library")]
    SidecarsNeedWriteAccess,
    #[error("a slicer needs a name, a program and at least one file type")]
    InvalidSlicer,
    #[error("{0}")]
    InvalidMeta(&'static str),
    #[error("unknown code {code:?} for {what}")]
    UnknownCode { what: &'static str, code: String },
}
