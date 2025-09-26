use clap::{Arg, ArgAction, Command};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

mod adif;
mod encoding;
mod entities;
mod errors;
mod mojibake;

use crate::encoding::OutputEncoding;
use crate::errors::TransadifError;

#[derive(Debug, Clone)]
pub struct Config {
    pub input_file: Option<PathBuf>,
    pub output_file: Option<PathBuf>,
    pub input_encoding: Option<String>,
    pub output_encoding: OutputEncoding,
    pub transcode: bool,
    pub replace_char: Option<char>,
    pub delete_incompatible: bool,
    pub ascii_transliterate: bool,
    pub strict_mode: bool,
    pub debug_qsos: Option<Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input_file: None,
            output_file: None,
            input_encoding: None,
            output_encoding: OutputEncoding::Utf8,
            transcode: false,
            replace_char: Some('?'),
            delete_incompatible: false,
            ascii_transliterate: false,
            strict_mode: false,
            debug_qsos: None,
        }
    }
}

fn main() -> Result<(), TransadifError> {
    let matches = Command::new("transadif")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Process ADIF files with proper encoding handling")
        .long_about("TransADIF - A command-line tool for processing ADIF files with intelligent encoding detection and correction.\n\nSupports automatic mojibake correction, field count reinterpretation, and multiple output encodings.")
        .arg(
            Arg::new("input")
                .help("Input ADIF file (reads from stdin if not specified)")
                .index(1)
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output file (writes to stdout if not specified)")
        )
        .arg(
            Arg::new("input-encoding")
                .short('i')
                .long("input-encoding")
                .value_name("ENCODING")
                .help("Suggested encoding for input file")
        )
        .arg(
            Arg::new("encoding")
                .short('e')
                .long("encoding")
                .value_name("ENCODING")
                .help("Output encoding (utf-8, iso-8859-1, windows-1252, ascii)")
                .default_value("utf-8")
        )
        .arg(
            Arg::new("transcode")
                .short('t')
                .long("transcode")
                .help("Transcode compatible characters")
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("replace")
                .short('r')
                .long("replace")
                .value_name("CHAR")
                .help("Replace incompatible characters with specified character")
                .default_value("?")
        )
        .arg(
            Arg::new("delete")
                .long("delete")
                .help("Delete incompatible characters")
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("ascii")
                .short('a')
                .long("ascii")
                .help("Transliterate to characters without diacritics")
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("strict")
                .short('s')
                .long("strict")
                .help("Strict mode - report errors instead of correcting")
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .value_name("QSO_NUMBERS")
                .help("Debug mode - print detailed information for specified QSOs (comma-separated, e.g., 1,3,5)")
                .action(ArgAction::Set)
        )
        .get_matches();

    let config = Config {
        input_file: matches.get_one::<String>("input").map(PathBuf::from),
        output_file: matches.get_one::<String>("output").map(PathBuf::from),
        input_encoding: matches.get_one::<String>("input-encoding").cloned(),
        output_encoding: parse_output_encoding(matches.get_one::<String>("encoding").unwrap())?,
        transcode: matches.get_flag("transcode"),
        replace_char: if matches.get_flag("delete") {
            None
        } else {
            matches.get_one::<String>("replace")
                .and_then(|s| s.chars().next())
        },
        delete_incompatible: matches.get_flag("delete"),
        ascii_transliterate: matches.get_flag("ascii"),
        strict_mode: matches.get_flag("strict"),
        debug_qsos: matches.get_one::<String>("debug")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect()),
    };

    process_adif_file(config)
}

fn parse_output_encoding(encoding: &str) -> Result<OutputEncoding, TransadifError> {
    match encoding.to_lowercase().as_str() {
        "utf-8" | "utf8" => Ok(OutputEncoding::Utf8),
        "iso-8859-1" | "iso8859-1" | "latin1" => Ok(OutputEncoding::Iso88591),
        "windows-1252" | "win1252" | "cp1252" => Ok(OutputEncoding::Windows1252),
        "ascii" | "us-ascii" => Ok(OutputEncoding::Ascii),
        _ => Err(TransadifError::InvalidEncoding(encoding.to_string())),
    }
}

fn process_adif_file(config: Config) -> Result<(), TransadifError> {
    // Read input
    let input_bytes = if let Some(input_file) = &config.input_file {
        fs::read(input_file)?
    } else {
        let mut buffer = Vec::new();
        io::stdin().read_to_end(&mut buffer)?;
        buffer
    };

    // Parse ADIF file
    let mut adif_file = adif::parse_adif(&input_bytes, &config)?;

    // Process and convert encodings
    adif_file.process_encodings(&config)?;

    // Debug output if requested
    if let Some(ref debug_qsos) = config.debug_qsos {
        print_debug_info(&adif_file, debug_qsos);
    }

    // Generate output
    let output_bytes = adif_file.generate_output(&config)?;

    // Write output
    if let Some(output_file) = &config.output_file {
        fs::write(output_file, output_bytes)?;
    } else {
        io::stdout().write_all(&output_bytes)?;
    }

    Ok(())
}

fn print_debug_info(adif_file: &adif::AdifFile, debug_qsos: &[String]) {
    eprintln!("=== DEBUG MODE ===");

    // Parse QSO numbers (support both numbers and "all")
    let mut qso_numbers = Vec::new();
    for qso_spec in debug_qsos {
        if qso_spec.to_lowercase() == "all" {
            // Debug all QSOs
            for i in 1..=adif_file.records.len() {
                qso_numbers.push(i);
            }
            break;
        } else if let Ok(num) = qso_spec.parse::<usize>() {
            qso_numbers.push(num);
        } else {
            eprintln!("Warning: Invalid QSO number '{}'", qso_spec);
        }
    }

    eprintln!("Total QSOs in file: {}", adif_file.records.len());
    eprintln!("Debugging QSOs: {:?}", qso_numbers);
    eprintln!();

    for qso_num in qso_numbers {
        if qso_num == 0 || qso_num > adif_file.records.len() {
            eprintln!("Warning: QSO {} does not exist (valid range: 1-{})", qso_num, adif_file.records.len());
            continue;
        }

        let record = &adif_file.records[qso_num - 1];
        eprintln!("=== QSO {} ===", qso_num);
        eprintln!("Fields: {}", record.fields.len());

        for (field_idx, field) in record.fields.iter().enumerate() {
            eprintln!("  Field {}: {}", field_idx + 1, field.name.to_uppercase());

            // Show original data info
            if !field.raw_data.is_empty() {
                eprintln!("    Original bytes: {} bytes", field.raw_data.len());
                eprintln!("    Original hex: {}", hex_preview(&field.raw_data));
            }

            // Show interpreted data
            eprintln!("    Interpreted data: {:?}", field.data);
            eprintln!("    Character count: {}", field.data.chars().count());
            eprintln!("    Byte count (UTF-8): {}", field.data.as_bytes().len());

            // Show excess data if any
            if !field.excess_data.is_empty() {
                eprintln!("    Excess data: {:?}", field.excess_data);
            }

            eprintln!();
        }

        if !record.excess_data.is_empty() {
            eprintln!("  Record excess data: {:?}", record.excess_data);
        }

        eprintln!();
    }
}

fn hex_preview(bytes: &[u8]) -> String {
    if bytes.len() <= 32 {
        bytes.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ")
    } else {
        let preview = bytes[..16].iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
        format!("{} ... ({} more bytes)", preview, bytes.len() - 16)
    }
}
