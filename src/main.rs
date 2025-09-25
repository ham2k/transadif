use clap::{Arg, Command};
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use transadif::{
    AdifFile, EncodingProcessor, EncodingOptions, OutputEncoding, Result,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let matches = Command::new("transadif")
        .version("0.1.0")
        .author("Ham2K")
        .about("A command-line tool for processing ADIF files with proper encoding handling")
        .arg(
            Arg::new("input")
                .help("Input ADIF file (reads from stdin if not specified)")
                .value_name("INPUT_FILE")
                .index(1),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .help("Output file (writes to stdout if not specified)")
                .value_name("OUTPUT_FILE"),
        )
        .arg(
            Arg::new("input-encoding")
                .short('i')
                .long("input-encoding")
                .help("Suggested encoding for the input file")
                .value_name("ENCODING"),
        )
        .arg(
            Arg::new("encoding")
                .short('e')
                .long("encoding")
                .help("Output encoding (UTF-8, ASCII, ISO-8859-1, Windows-1252)")
                .value_name("ENCODING")
                .default_value("UTF-8"),
        )
        .arg(
            Arg::new("transcode")
                .short('t')
                .long("transcode")
                .help("Transcode compatible characters")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("replace")
                .short('r')
                .long("replace")
                .help("Replace incompatible characters with specified character")
                .value_name("CHAR")
                .default_value("?"),
        )
        .arg(
            Arg::new("delete")
                .short('d')
                .long("delete")
                .help("Delete incompatible characters")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("ascii")
                .short('a')
                .long("ascii")
                .help("Transliterate to characters without diacritics")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("strict")
                .short('s')
                .long("strict")
                .help("Strict mode - do not correct invalid characters or field counts")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Parse command line arguments
    let input_file = matches.get_one::<String>("input");
    let output_file = matches.get_one::<String>("output");
    let input_encoding = matches.get_one::<String>("input-encoding");
    let output_encoding_str = matches.get_one::<String>("encoding").unwrap();
    let transcode = matches.get_flag("transcode");
    let replace_char_str = matches.get_one::<String>("replace").unwrap();
    let delete_incompatible = matches.get_flag("delete");
    let ascii_transliterate = matches.get_flag("ascii");
    let strict_mode = matches.get_flag("strict");

    // Parse output encoding
    let output_encoding = match output_encoding_str.to_lowercase().as_str() {
        "utf-8" | "utf8" => OutputEncoding::Utf8,
        "ascii" | "us-ascii" => OutputEncoding::Ascii,
        encoding_name => OutputEncoding::CodePage(encoding_name.to_string()),
    };

    // Parse replace character
    let replace_char = if delete_incompatible {
        None
    } else {
        replace_char_str.chars().next()
    };

    // Create encoding options
    let options = EncodingOptions {
        output_encoding,
        transcode,
        replace_char,
        delete_incompatible,
        ascii_transliterate,
        strict_mode,
    };

    // Read input data
    let input_data = read_input(input_file)?;

    // Parse ADIF file
    let mut file = AdifFile::parse(&input_data)?;

    // Process encoding
    let mut processor = EncodingProcessor::new(options);
    file = processor.process_file(file, input_encoding.map(|s| s.as_str()))?;

    // Show warnings
    for warning in processor.get_warnings() {
        eprintln!("Warning: {}", warning);
    }

    // Write output
    write_output(&file, &processor, output_file)?;

    Ok(())
}

fn read_input(input_file: Option<&String>) -> Result<Vec<u8>> {
    let mut data = Vec::new();

    match input_file {
        Some(filename) => {
            let file = File::open(filename)?;
            let mut reader = BufReader::new(file);
            reader.read_to_end(&mut data)?;
        }
        None => {
            let stdin = io::stdin();
            let mut handle = stdin.lock();
            handle.read_to_end(&mut data)?;
        }
    }

    Ok(data)
}

fn write_output(
    file: &AdifFile,
    processor: &EncodingProcessor,
    output_file: Option<&String>,
) -> Result<()> {
    match output_file {
        Some(filename) => {
            let file_handle = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(filename)?;
            let mut writer = BufWriter::new(file_handle);
            file.write_to(&mut writer, processor)?;
            writer.flush()?;
        }
        None => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            file.write_to(&mut handle, processor)?;
            handle.flush()?;
        }
    }

    Ok(())
}
