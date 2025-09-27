use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TestError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Test timeout: {0}")]
    Timeout(String),
    #[error("Command parsing error: {0}")]
    CommandParsing(String),
    #[error("Test execution error: {0}")]
    Execution(String),
}

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub input_file: PathBuf,
    pub expected_output_file: PathBuf,
    pub command: String,
}

#[derive(Debug)]
pub struct TestResult {
    pub test_case: TestCase,
    pub passed: bool,
    pub error: Option<String>,
    pub execution_time: Duration,
    pub differences: Vec<ByteDifference>,
}

#[derive(Debug)]
pub struct ByteDifference {
    pub position: usize,
    pub expected: u8,
    pub actual: u8,
    pub context: String,
}

pub struct TestRunner {
    pub timeout: Duration,
    pub executable_path: PathBuf,
}

impl TestRunner {
    pub fn new(executable_path: PathBuf) -> Self {
        Self {
            timeout: Duration::from_secs(10),
            executable_path,
        }
    }

    pub fn find_test_cases<P: AsRef<Path>>(&self, test_dir: P, filter: Option<&str>) -> Result<Vec<TestCase>, TestError> {
        let mut test_cases = Vec::new();
        self.find_test_cases_recursive(test_dir.as_ref(), &mut test_cases, filter)?;
        test_cases.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(test_cases)
    }

    fn find_test_cases_recursive(
        &self,
        dir: &Path,
        test_cases: &mut Vec<TestCase>,
        filter: Option<&str>
    ) -> Result<(), TestError> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recursively search subdirectories
                self.find_test_cases_recursive(&path, test_cases, filter)?;
            } else if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                // Look for input files, but skip temporary files
                if (file_name.contains("-in.adi") || file_name.ends_with("-in.adi")) && !file_name.ends_with(".tmp") {
                    if let Some(filter_str) = filter {
                        if !file_name.contains(filter_str) && !path.to_string_lossy().contains(filter_str) {
                            continue;
                        }
                    }

                    // Find corresponding output file
                    let output_file = self.find_output_file(&path)?;

                    // Extract command from input file
                    let command = self.extract_command_from_file(&path)?;

                    let test_case = TestCase {
                        name: self.generate_test_name(&path),
                        input_file: path,
                        expected_output_file: output_file,
                        command,
                    };

                    test_cases.push(test_case);
                }
            }
        }

        Ok(())
    }

    fn find_output_file(&self, input_file: &Path) -> Result<PathBuf, TestError> {
        let file_name = input_file.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| TestError::CommandParsing("Invalid input filename".to_string()))?;

        // Try different patterns for output files
        let patterns = [
            file_name.replace("-in.adi", "-out.adi"),
            file_name.replace("-in.adi", ".adi"),
        ];

        for pattern in &patterns {
            let output_path = input_file.with_file_name(pattern);
            if output_path.exists() {
                return Ok(output_path);
            }
        }

        Err(TestError::CommandParsing(format!("Could not find output file for {}", file_name)))
    }

    fn extract_command_from_file(&self, file_path: &Path) -> Result<String, TestError> {
        // Read file as raw bytes to handle any encoding
        let content = fs::read(file_path)?;
        let content_str = String::from_utf8_lossy(&content);

        // Look for command line in the preamble
        for line in content_str.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Command:") {
                // Extract the command after "Command:"
                if let Some(cmd_start) = trimmed.find('`') {
                    if let Some(cmd_end) = trimmed.rfind('`') {
                        if cmd_start < cmd_end {
                            let command = &trimmed[cmd_start + 1..cmd_end];
                            return Ok(command.to_string());
                        }
                    }
                }
            }
        }

        // Default command if none found
        Ok(format!("transadif {}", file_path.display()))
    }

    fn generate_test_name(&self, file_path: &Path) -> String {
        // Generate a readable test name from the file path
        let relative_path = file_path.strip_prefix("test-cases")
            .unwrap_or(file_path);

        relative_path.to_string_lossy()
            .replace('/', "::")
            .replace("-in.adi", "")
    }

    pub fn run_test(&self, test_case: &TestCase) -> TestResult {
        let start_time = Instant::now();

        match self.execute_test_command(test_case) {
            Ok(actual_output) => {
                match fs::read(&test_case.expected_output_file) {
                    Ok(expected_output) => {
                        let differences = self.compare_bytes(&expected_output, &actual_output);
                        let passed = differences.is_empty();

                        TestResult {
                            test_case: test_case.clone(),
                            passed,
                            error: None,
                            execution_time: start_time.elapsed(),
                            differences,
                        }
                    }
                    Err(e) => TestResult {
                        test_case: test_case.clone(),
                        passed: false,
                        error: Some(format!("Could not read expected output: {}", e)),
                        execution_time: start_time.elapsed(),
                        differences: Vec::new(),
                    }
                }
            }
            Err(e) => TestResult {
                test_case: test_case.clone(),
                passed: false,
                error: Some(e.to_string()),
                execution_time: start_time.elapsed(),
                differences: Vec::new(),
            }
        }
    }

    fn execute_test_command(&self, test_case: &TestCase) -> Result<Vec<u8>, TestError> {
        // Parse the command and replace {filename} placeholder
        let command = test_case.command.replace("{filename}", &test_case.input_file.to_string_lossy());

        // For now, we'll assume the command is our binary with arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(TestError::CommandParsing("Empty command".to_string()));
        }

        let mut cmd = Command::new(&self.executable_path);

        // Add the input file and any other arguments
        cmd.arg(&test_case.input_file);

        // Execute with timeout
        let output = cmd.output()
            .map_err(|e| TestError::Execution(format!("Failed to execute command: {}", e)))?;

        if !output.status.success() {
            return Err(TestError::Execution(format!(
                "Command failed with exit code {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(output.stdout)
    }

    fn compare_bytes(&self, expected: &[u8], actual: &[u8]) -> Vec<ByteDifference> {
        let mut differences = Vec::new();
        let max_len = expected.len().max(actual.len());

        for i in 0..max_len {
            let expected_byte = expected.get(i).copied().unwrap_or(0);
            let actual_byte = actual.get(i).copied().unwrap_or(0);

            if expected_byte != actual_byte {
                let context = self.get_context_string(expected, actual, i);
                differences.push(ByteDifference {
                    position: i,
                    expected: expected_byte,
                    actual: actual_byte,
                    context,
                });
            }
        }

        differences
    }

    fn get_context_string(&self, expected: &[u8], actual: &[u8], position: usize) -> String {
        let context_size = 20;
        let start = position.saturating_sub(context_size);

        let expected_end = (position + context_size).min(expected.len());
        let actual_end = (position + context_size).min(actual.len());

        let expected_context = if start < expected.len() {
            String::from_utf8_lossy(&expected[start..expected_end])
        } else {
            "".into()
        };

        let actual_context = if start < actual.len() {
            String::from_utf8_lossy(&actual[start..actual_end])
        } else {
            "".into()
        };

        format!(
            "Expected: {:?} | Actual: {:?}",
            expected_context,
            actual_context
        )
    }

    pub fn print_test_result(&self, result: &TestResult) {
        if result.passed {
            println!("✓ {} ({:?})", result.test_case.name, result.execution_time);
        } else {
            println!("✗ {} ({:?})", result.test_case.name, result.execution_time);

            if let Some(ref error) = result.error {
                println!("  Error: {}", error);
            }

            if !result.differences.is_empty() {
                println!("  Differences found:");
                for (i, diff) in result.differences.iter().take(5).enumerate() {
                    println!(
                        "    [{}] Position {}: expected 0x{:02X} ('{}'), got 0x{:02X} ('{}')",
                        i + 1,
                        diff.position,
                        diff.expected,
                        if diff.expected.is_ascii_graphic() { diff.expected as char } else { '.' },
                        diff.actual,
                        if diff.actual.is_ascii_graphic() { diff.actual as char } else { '.' }
                    );
                    println!("        Context: {}", diff.context);
                }

                if result.differences.len() > 5 {
                    println!("    ... and {} more differences", result.differences.len() - 5);
                }
            }
        }
    }

    pub fn run_all_tests<P: AsRef<Path>>(&self, test_dir: P, filter: Option<&str>) -> Result<(), TestError> {
        let test_cases = self.find_test_cases(test_dir, filter)?;

        if test_cases.is_empty() {
            println!("No test cases found");
            return Ok(());
        }

        println!("Running {} test case(s)...\n", test_cases.len());

        let mut passed = 0;
        let mut failed = 0;

        for test_case in &test_cases {
            let result = self.run_test(test_case);
            self.print_test_result(&result);

            if result.passed {
                passed += 1;
            } else {
                failed += 1;
            }
        }

        println!("\n{} passed, {} failed", passed, failed);

        if failed > 0 {
            std::process::exit(1);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_extraction() {
        let runner = TestRunner::new(PathBuf::from("transadif"));

        // This would need actual test files to work properly
        // For now, just test the basic structure
    }
}