use std::io;

#[derive(Debug, thiserror::Error)]
pub enum TransadifError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid encoding: {0}")]
    InvalidEncoding(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Encoding conversion error: {0}")]
    EncodingError(String),

    #[error("Field count mismatch in field '{field}': expected {expected}, got {actual}")]
    FieldCountMismatch {
        field: String,
        expected: usize,
        actual: usize,
    },

    #[error("Invalid field format: {0}")]
    InvalidFieldFormat(String),

    #[error("Invalid character in strict mode: {0}")]
    InvalidCharacter(String),

    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),
}
