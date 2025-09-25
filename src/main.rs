use clap::{Arg, Command};
use std::fs;
use std::io::{self, Read, Write};

mod adif;
mod encoding;
mod error;
mod output;
pub mod test_runner;

use crate::adif::AdifFile;
use crate::encoding::EncodingOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("transadif")
        .version("0.1.0")
        .about("A command-line tool for processing and transcoding ADIF files")
        .arg(
            Arg::new("input")
                .help("Input ADIF file (use '-' for stdin)")
                .value_name("INPUT_FILE")
                .index(1),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("OUTPUT_FILE")
                .help("Write output to file (default: stdout)"),
        )
        .arg(
            Arg::new("input-encoding")
                .short('i')
                .long("input-encoding")
                .value_name("ENCODING")
                .help("Suggested encoding for input file (ascii, iso-8859-1, utf-8)"),
        )
        .arg(
            Arg::new("encoding")
                .short('e')
                .long("encoding")
                .value_name("ENCODING")
                .help("Output encoding (ascii, iso-8859-1, utf-8)")
                .default_value("utf-8"),
        )
        .arg(
            Arg::new("transcode")
                .short('t')
                .long("transcode")
                .action(clap::ArgAction::SetTrue)
                .help("Transcode compatible characters"),
        )
        .arg(
            Arg::new("replace")
                .short('r')
                .long("replace")
                .value_name("CHAR")
                .help("Replace incompatible characters with specified character")
                .default_value("?"),
        )
        .arg(
            Arg::new("delete")
                .short('d')
                .long("delete")
                .action(clap::ArgAction::SetTrue)
                .help("Delete incompatible characters"),
        )
        .arg(
            Arg::new("ascii")
                .short('a')
                .long("ascii")
                .action(clap::ArgAction::SetTrue)
                .help("Transliterate to characters without diacritics"),
        )
        .arg(
            Arg::new("strict")
                .short('s')
                .long("strict")
                .action(clap::ArgAction::SetTrue)
                .help("Strict mode - report errors instead of correcting"),
        )
        .get_matches();

    // Read input
    let input_data = if let Some(input_file) = matches.get_one::<String>("input") {
        if input_file == "-" {
            let mut buffer = Vec::new();
            io::stdin().read_to_end(&mut buffer)?;
            buffer
        } else {
            fs::read(input_file)?
        }
    } else {
        let mut buffer = Vec::new();
        io::stdin().read_to_end(&mut buffer)?;
        buffer
    };

    // Parse encoding options
    let encoding_opts = EncodingOptions {
        input_encoding: matches.get_one::<String>("input-encoding").cloned(),
        output_encoding: matches.get_one::<String>("encoding").unwrap().clone(),
        transcode: matches.get_flag("transcode"),
        replace_char: matches.get_one::<String>("replace").unwrap().clone(),
        delete_incompatible: matches.get_flag("delete"),
        ascii_transliterate: matches.get_flag("ascii"),
        strict_mode: matches.get_flag("strict"),
    };

    // Parse ADIF file
    let mut adif_file = AdifFile::parse(&input_data, &encoding_opts)?;

    // Process and generate output
    let output_data = output::generate_adif(&mut adif_file, &encoding_opts)?;

    // Write output
    if let Some(output_file) = matches.get_one::<String>("output") {
        fs::write(output_file, output_data)?;
    } else {
        io::stdout().write_all(&output_data)?;
    }

    Ok(())
}
