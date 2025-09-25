use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransAdifError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Encoding error: {0}")]
    Encoding(String),
    
    #[error("Parse error at position {pos}: {msg}")]
    Parse { pos: usize, msg: String },
    
    #[error("Invalid field: {0}")]
    InvalidField(String),
    
    #[error("Invalid encoding: {0}")]
    InvalidEncoding(String),
    
    #[error("Strict mode violation: {0}")]
    StrictMode(String),
}

pub type Result<T> = std::result::Result<T, TransAdifError>;
