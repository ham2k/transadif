use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransadifError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Invalid field: {0}")]
    InvalidField(String),

    #[error("Invalid header: {0}")]
    InvalidHeader(String),

    #[error("Test error: {0}")]
    Test(String),
}

pub type Result<T> = std::result::Result<T, TransadifError>;
