use clap::{Arg, Command};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use transadif::{Result, TransadifError};

#[derive(Debug)]
struct TestCase {
    name: String,
    input_file: PathBuf,
    expected_output_file: PathBuf,
    command_line: String,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let matches = Command::new("test_runner")
        .version("0.1.0")
        .author("Ham2K")
        .about("Test runner for transadif")
        .arg(
            Arg::new("test-dir")
                .short('d')
                .long("test-dir")
                .help("Directory containing test cases")
                .value_name("DIR")
                .default_value("test-cases"),
        )
        .arg(
            Arg::new("filter")
                .short('f')
                .long("filter")
                .help("Run only tests matching this filter")
                .value_name("FILTER"),
        )
        .arg(
            Arg::new("binary")
                .short('b')
                .long("binary")
                .help("Path to transadif binary")
                .value_name("BINARY")
                .default_value("target/debug/transadif"),
        )
        .get_matches();

    let test_dir = matches.get_one::<String>("test-dir").unwrap();
    let filter = matches.get_one::<String>("filter");
    let binary_path = matches.get_one::<String>("binary").unwrap();

    // Discover test cases
    let test_cases = discover_test_cases(test_dir, filter)?;

    if test_cases.is_empty() {
        println!("No test cases found");
        return Ok(());
    }

    println!("Found {} test cases", test_cases.len());
    println!();

    let mut passed = 0;
    let mut failed = 0;

    for test_case in test_cases {
        print!("Running test '{}' ... ", test_case.name);

        match run_test_case(&test_case, binary_path) {
            Ok(()) => {
                println!("PASSED");
                passed += 1;
            }
            Err(e) => {
                println!("FAILED");
                eprintln!("  Error: {}", e);
                failed += 1;
            }
        }
    }

    println!();
    println!("Results: {} passed, {} failed", passed, failed);

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn discover_test_cases(test_dir: &str, filter: Option<&String>) -> Result<Vec<TestCase>> {
    let test_path = Path::new(test_dir);
    if !test_path.exists() {
        return Err(TransadifError::Test(format!(
            "Test directory '{}' does not exist",
            test_dir
        )));
    }

    let mut test_cases = Vec::new();
    discover_test_cases_recursive(test_path, &mut test_cases, filter)?;

    test_cases.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(test_cases)
}

fn discover_test_cases_recursive(
    dir: &Path,
    test_cases: &mut Vec<TestCase>,
    filter: Option<&String>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            discover_test_cases_recursive(&path, test_cases, filter)?;
        } else if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if filename.ends_with("-in.adi") {
                // This is an input file, look for corresponding output file
                let output_filename = filename.replace("-in.adi", "-out.adi");
                let output_path = path.parent().unwrap().join(output_filename);

                if output_path.exists() {
                    let test_name = filename.replace("-in.adi", "");

                    // Apply filter if specified
                    if let Some(filter_str) = filter {
                        if !test_name.contains(filter_str) && !path.to_string_lossy().contains(filter_str) {
                            continue;
                        }
                    }

                    // Extract command line from input file
                    if let Ok(command_line) = extract_command_line(&path) {
                        let parent_dir = path.parent().unwrap().file_name().unwrap_or_default().to_string_lossy();
                        test_cases.push(TestCase {
                            name: format!("{}:{}", parent_dir, test_name),
                            input_file: path.clone(),
                            expected_output_file: output_path,
                            command_line,
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

fn extract_command_line(input_file: &Path) -> Result<String> {
    // Read file as raw bytes to handle any encoding
    let data = fs::read(input_file)?;

    // Convert to string, replacing invalid UTF-8 with replacement characters
    let content = String::from_utf8_lossy(&data);

    // Look for command line in preamble (before <eoh>)
    let eoh_regex = Regex::new(r"(?i)<eoh>").unwrap();
    let preamble = if let Some(eoh_match) = eoh_regex.find(&content) {
        &content[..eoh_match.start()]
    } else {
        &content
    };

    // Look for command line patterns
    let command_patterns = [
        Regex::new(r"(?m)^#\s*transadif\s+(.+)$").unwrap(),
        Regex::new(r"(?m)^#\s*command:\s*transadif\s+(.+)$").unwrap(),
        Regex::new(r"(?m)^#\s*cmd:\s*transadif\s+(.+)$").unwrap(),
        Regex::new(r"Command:\s*`transadif\s+([^`]+)`").unwrap(),
        Regex::new(r"transadif\s+([^\r\n]+)").unwrap(),
    ];

    for pattern in &command_patterns {
        if let Some(captures) = pattern.captures(preamble) {
            let command_args = captures.get(1).unwrap().as_str().trim();
            return Ok(command_args.to_string());
        }
    }

    // Default command if none found
    Ok(String::new())
}

fn run_test_case(test_case: &TestCase, binary_path: &str) -> Result<()> {
    // Parse command line arguments
    let mut args = shell_words::split(&test_case.command_line)
        .map_err(|e| TransadifError::Test(format!("Failed to parse command line: {}", e)))?;

    // Replace input file placeholder with actual input file
    for arg in &mut args {
        if arg == "<input>" || arg == "{input}" || arg == "{filename}" {
            *arg = test_case.input_file.to_string_lossy().to_string();
        }
    }

    // Add input file if not already specified (and no {filename} placeholder was used)
    if !args.iter().any(|arg| !arg.starts_with('-')) && !test_case.command_line.contains("{filename}") {
        args.push(test_case.input_file.to_string_lossy().to_string());
    }

    // Run the command
    println!("  Command: {} {}", binary_path, args.join(" "));
    let mut cmd = ProcessCommand::new(binary_path);
    cmd.args(&args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output()
        .map_err(|e| TransadifError::Test(format!("Failed to execute transadif: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TransadifError::Test(format!(
            "transadif exited with error: {}",
            stderr
        )));
    }

    // Read expected output
    let expected_output = fs::read(&test_case.expected_output_file)
        .map_err(|e| TransadifError::Test(format!(
            "Failed to read expected output file: {}",
            e
        )))?;

    // Compare outputs byte-by-byte
    let actual_output = output.stdout;

    if actual_output != expected_output {
        // Create a detailed diff message
        let expected_str = String::from_utf8_lossy(&expected_output);
        let actual_str = String::from_utf8_lossy(&actual_output);

        return Err(TransadifError::Test(format!(
            "Output mismatch:\nExpected ({} bytes):\n{}\n\nActual ({} bytes):\n{}\n",
            expected_output.len(),
            expected_str,
            actual_output.len(),
            actual_str
        )));
    }

    Ok(())
}

// Simple shell word splitting implementation
mod shell_words {
    pub fn split(input: &str) -> Result<Vec<String>, String> {
        let mut words = Vec::new();
        let mut current_word = String::new();
        let mut chars = input.chars().peekable();
        let mut in_quotes = false;
        let mut quote_char = '"';

        while let Some(ch) = chars.next() {
            match ch {
                '"' | '\'' if !in_quotes => {
                    in_quotes = true;
                    quote_char = ch;
                }
                ch if in_quotes && ch == quote_char => {
                    in_quotes = false;
                }
                ' ' | '\t' if !in_quotes => {
                    if !current_word.is_empty() {
                        words.push(current_word);
                        current_word = String::new();
                    }
                }
                ch => {
                    current_word.push(ch);
                }
            }
        }

        if in_quotes {
            return Err("Unclosed quote".to_string());
        }

        if !current_word.is_empty() {
            words.push(current_word);
        }

        Ok(words)
    }
}
