use transadif::{adif, encoding, cli, output};

use clap::Parser;
use cli::Cli;
use encoding::AdifEncoding;
use output::{OutputFormatter, DebugFormatter};
use std::fs;
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    // Read input
    let input_data = if let Some(input_path) = &args.input {
        fs::read(input_path)?
    } else {
        let mut buffer = Vec::new();
        io::stdin().read_to_end(&mut buffer)?;
        buffer
    };

    // Parse ADIF file
    let adif = adif::AdifFile::parse(&input_data)?;

    // Handle debug mode
    let debug_qsos = args.parse_debug_qsos();
    if !debug_qsos.is_empty() {
        DebugFormatter::print_qso_debug(&adif, &debug_qsos);
        return Ok(());
    }

    // Determine input and output encodings
    let input_encoding = if let Some(encoding_str) = &args.input_encoding {
        Some(AdifEncoding::from_str(encoding_str)?)
    } else {
        adif.encoding.as_ref().and_then(|e| AdifEncoding::from_str(e).ok())
    };

    let output_encoding = AdifEncoding::from_str(&args.encoding)?;

    // Create formatter
    let replacement_char = if args.delete {
        None
    } else {
        Some(args.replace)
    };

    let formatter = OutputFormatter::new(
        input_encoding,
        output_encoding,
        args.strict,
        replacement_char,
        args.delete,
        args.ascii,
    );

    // Write output
    if let Some(output_path) = &args.output {
        let mut file = fs::File::create(output_path)?;
        formatter.format_adif(&adif, &mut file)?;
    } else {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        formatter.format_adif(&adif, &mut handle)?;
    }

    Ok(())
}
