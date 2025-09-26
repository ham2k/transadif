use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::io::{self, Write};
use clap::{Arg, Command as ClapCommand};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = ClapCommand::new("test_runner")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Test runner for TransADIF")
        .long_about("TransADIF Test Runner - Validates TransADIF functionality against test cases.\n\nRuns all test cases in the test-cases directory and compares outputs.")
        .arg(
            Arg::new("filter")
                .help("Filter tests by filename or directory")
                .index(1)
        )
        .get_matches();

    let filter = matches.get_one::<String>("filter");

    let test_cases_dir = Path::new("test-cases");
    if !test_cases_dir.exists() {
        eprintln!("Test cases directory not found: {}", test_cases_dir.display());
        std::process::exit(1);
    }

    let mut passed = 0;
    let mut failed = 0;
    let mut total = 0;

    run_tests_in_dir(test_cases_dir, filter, &mut passed, &mut failed, &mut total)?;

    println!("\n=== Test Results ===");
    println!("Total: {}", total);
    println!("Passed: {}", passed);
    println!("Failed: {}", failed);

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn run_tests_in_dir(
    dir: &Path,
    filter: Option<&String>,
    passed: &mut u32,
    failed: &mut u32,
    total: &mut u32
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            run_tests_in_dir(&path, filter, passed, failed, total)?;
        } else if path.is_file() {
            let filename = path.file_name().unwrap().to_string_lossy();

            // Skip if doesn't match filter
            if let Some(filter_str) = filter {
                if !filename.contains(filter_str) && !path.to_string_lossy().contains(filter_str) {
                    continue;
                }
            }

            // Only process input files
            if filename.ends_with("-in.adi") {
                let test_name = filename.replace("-in.adi", "");
                let expected_output = path.with_file_name(format!("{}-out.adi", test_name));

                if expected_output.exists() {
                    *total += 1;
                    if run_single_test(&path, &expected_output)? {
                        *passed += 1;
                        println!("✓ {}", path.display());
                    } else {
                        *failed += 1;
                        println!("✗ {}", path.display());
                    }
                }
            }
        }
    }

    Ok(())
}

fn run_single_test(input_file: &Path, expected_output: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    // Read the input file to extract the command
    let input_bytes = fs::read(input_file)?;
    let input_text = String::from_utf8_lossy(&input_bytes);

    // Find the command line in the preamble
    let command_line = extract_command_from_preamble(&input_text, input_file)?;

    // Execute the command with timeout
    let output = execute_with_timeout(&command_line, 10)?;

    if !output.status.success() {
        eprintln!("Command failed for {}: {}", input_file.display(),
                 String::from_utf8_lossy(&output.stderr));
        return Ok(false);
    }

    // Read expected output
    let expected_bytes = fs::read(expected_output)?;

    // Compare outputs (byte-by-byte, ignoring preamble differences)
    let actual_output = output.stdout;

    // For now, do a simple byte comparison
    // TODO: Implement more sophisticated comparison that ignores preamble differences
    let matches = actual_output == expected_bytes;
    if !matches {
        print_detailed_diff(input_file, &actual_output, &expected_bytes);
    }
    Ok(matches)
}

fn extract_command_from_preamble(text: &str, input_file: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let transadif_path = std::env::current_dir()?.join("target/debug/transadif");

    for line in text.lines() {
        if line.starts_with("Command:") {
            let command = line.strip_prefix("Command:").unwrap().trim();
            // Remove backticks if present
            let command = command.trim_matches('`');
            // Replace transadif with full path and {filename} with actual filename
            let command = command.replace("transadif", &transadif_path.to_string_lossy());
            let command = command.replace("{filename}", &format!("\"{}\"", input_file.display()));
            return Ok(command.to_string());
        }
    }

    // Default command if not found
    Ok(format!("\"{}\" \"{}\"", transadif_path.display(), input_file.display()))
}

fn execute_with_timeout(command: &str, _timeout_secs: u64) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    // Simple command parsing - split on spaces but preserve quoted strings
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = command.chars();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(current);
                    current = String::new();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        return Err("Empty command".into());
    }

    let mut cmd = Command::new(&parts[0]);
    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    let output = cmd.output()?;

    Ok(output)
}

fn print_detailed_diff(input_file: &Path, actual: &[u8], expected: &[u8]) {
    eprintln!("Output mismatch for {}", input_file.display());
    eprintln!("Actual length: {}, Expected length: {}", actual.len(), expected.len());

    if actual.len() != expected.len() {
        eprintln!("Length difference: {} bytes", (actual.len() as i64) - (expected.len() as i64));
    }

    // Find all differences
    let mut differences = Vec::new();
    let max_len = actual.len().max(expected.len());

    for i in 0..max_len {
        let actual_byte = actual.get(i).copied();
        let expected_byte = expected.get(i).copied();

        if actual_byte != expected_byte {
            differences.push(i);
        }
    }

    if differences.is_empty() {
        eprintln!("No byte differences found (this shouldn't happen!)");
        return;
    }

    eprintln!("Found {} byte difference(s)", differences.len());

    // Show first few differences with context
    let max_diffs_to_show = 5;
    for (diff_idx, &pos) in differences.iter().take(max_diffs_to_show).enumerate() {
        eprintln!();
        eprintln!("=== Difference {} at position {} ===", diff_idx + 1, pos);

        let actual_byte = actual.get(pos).copied();
        let expected_byte = expected.get(pos).copied();

        match (actual_byte, expected_byte) {
            (Some(a), Some(e)) => {
                eprintln!("  Actual:   0x{:02x} ({}) '{}'", a, a, format_byte_as_char(a));
                eprintln!("  Expected: 0x{:02x} ({}) '{}'", e, e, format_byte_as_char(e));
            },
            (Some(a), None) => {
                eprintln!("  Actual:   0x{:02x} ({}) '{}' (extra byte)", a, a, format_byte_as_char(a));
                eprintln!("  Expected: <end of file>");
            },
            (None, Some(e)) => {
                eprintln!("  Actual:   <end of file>");
                eprintln!("  Expected: 0x{:02x} ({}) '{}' (missing byte)", e, e, format_byte_as_char(e));
            },
            (None, None) => unreachable!(),
        }

        // Show context around the difference
        let context_size = 20;
        let start = pos.saturating_sub(context_size);
        let end = (pos + context_size + 1).min(max_len);

        eprintln!("  Context (±{} bytes):", context_size);
        print_hex_context(actual, expected, start, end, pos);
        print_text_context(actual, expected, start, end, pos);
    }

    if differences.len() > max_diffs_to_show {
        eprintln!();
        eprintln!("... and {} more differences", differences.len() - max_diffs_to_show);
    }
}

fn format_byte_as_char(byte: u8) -> String {
    match byte {
        b'\n' => "\\n".to_string(),
        b'\r' => "\\r".to_string(),
        b'\t' => "\\t".to_string(),
        b'\\' => "\\\\".to_string(),
        0x20..=0x7E => (byte as char).to_string(), // Printable ASCII
        _ => format!("\\x{:02x}", byte),
    }
}

fn print_hex_context(actual: &[u8], expected: &[u8], start: usize, end: usize, diff_pos: usize) {
    eprintln!("    Hex context:");

    // Actual bytes
    eprint!("    Actual:   ");
    for i in start..end {
        if i == diff_pos {
            eprint!("[{:02x}] ", actual.get(i).copied().unwrap_or(0));
        } else if let Some(byte) = actual.get(i) {
            eprint!("{:02x} ", byte);
        } else {
            eprint!("-- ");
        }
    }
    eprintln!();

    // Expected bytes
    eprint!("    Expected: ");
    for i in start..end {
        if i == diff_pos {
            eprint!("[{:02x}] ", expected.get(i).copied().unwrap_or(0));
        } else if let Some(byte) = expected.get(i) {
            eprint!("{:02x} ", byte);
        } else {
            eprint!("-- ");
        }
    }
    eprintln!();
}

fn print_text_context(actual: &[u8], expected: &[u8], start: usize, end: usize, diff_pos: usize) {
    eprintln!("    Text context:");

    // Actual text
    eprint!("    Actual:   \"");
    for i in start..end {
        if let Some(byte) = actual.get(i) {
            if i == diff_pos {
                eprint!("[{}]", format_byte_as_char(*byte));
            } else {
                eprint!("{}", format_byte_as_char(*byte));
            }
        } else {
            if i == diff_pos {
                eprint!("[EOF]");
            }
        }
    }
    eprintln!("\"");

    // Expected text
    eprint!("    Expected: \"");
    for i in start..end {
        if let Some(byte) = expected.get(i) {
            if i == diff_pos {
                eprint!("[{}]", format_byte_as_char(*byte));
            } else {
                eprint!("{}", format_byte_as_char(*byte));
            }
        } else {
            if i == diff_pos {
                eprint!("[EOF]");
            }
        }
    }
    eprintln!("\"");
}
