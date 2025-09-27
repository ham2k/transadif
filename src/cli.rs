use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "transadif")]
#[command(about = "Command-line tool for processing ADIF files with proper encoding handling")]
#[command(version = "0.1.0")]
pub struct Cli {
    /// Input ADIF file (reads from stdin if not specified)
    pub input: Option<PathBuf>,

    /// Output file (writes to stdout if not specified)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Suggested encoding for the input file
    #[arg(short = 'i', long)]
    pub input_encoding: Option<String>,

    /// Encoding for the output file
    #[arg(short, long, default_value = "UTF-8")]
    pub encoding: String,

    /// Transcode compatible characters
    #[arg(short, long)]
    pub transcode: bool,

    /// Replace incompatible characters with specified character
    #[arg(short, long, default_value = "?")]
    pub replace: char,

    /// Delete incompatible characters instead of replacing them
    #[arg(long)]
    pub delete: bool,

    /// Transliterate to characters without diacritics (ASCII mode)
    #[arg(short, long)]
    pub ascii: bool,

    /// Strict mode - do not correct invalid characters or field counts
    #[arg(short, long)]
    pub strict: bool,

    /// Debug mode - print contents of specified QSOs (comma-separated)
    #[arg(short, long)]
    pub debug: Option<String>,
}

impl Cli {
    pub fn parse_debug_qsos(&self) -> Vec<usize> {
        if let Some(ref debug_str) = self.debug {
            debug_str
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect()
        } else {
            Vec::new()
        }
    }
}