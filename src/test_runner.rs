use std::fs;
use std::path::Path;
use std::process::Command;
use std::str;

#[derive(Debug)]
pub struct TestCase {
    pub name: String,
    pub input_file: String,
    pub expected_output_file: String,
    pub command_args: Vec<String>,
}

impl TestCase {
    pub fn from_input_file(input_path: &Path) -> Option<Self> {
        let input_file = input_path.to_string_lossy().to_string();
        let file_name = input_path.file_name()?.to_string_lossy();
        
        // Parse test case name from filename (e.g., "001-in-plain-ascii.adi" -> "001")
        let name = file_name.split('-').next()?.to_string();
        
        // Construct expected output filename
        let expected_output_file = input_file.replace("-in-", "-out-");
        
        // Read the input file to extract command line
        let content = fs::read_to_string(input_path).ok()?;
        let command_args = Self::extract_command_args(&content)?;
        
        Some(TestCase {
            name,
            input_file,
            expected_output_file,
            command_args,
        })
    }
    
    fn extract_command_args(content: &str) -> Option<Vec<String>> {
        // Look for "Command: `transadif {filename}`" pattern in the first few lines
        for line in content.lines().take(10) {
            if line.trim().starts_with("Command:") {
                let command_part = line.split("Command:").nth(1)?.trim();
                if command_part.starts_with('`') && command_part.ends_with('`') {
                    let command = &command_part[1..command_part.len()-1];
                    // Parse command arguments
                    let parts: Vec<&str> = command.split_whitespace().collect();
                    if parts.len() >= 2 && parts[0] == "transadif" {
                        return Some(parts[1..].iter().map(|s| {
                            if *s == "{filename}" {
                                "INPUT_FILE".to_string()
                            } else {
                                s.to_string()
                            }
                        }).collect());
                    }
                }
            }
        }
        None
    }
    
    pub fn run(&self, binary_path: &Path) -> Result<TestResult, Box<dyn std::error::Error>> {
        // Replace INPUT_FILE placeholder with actual input file path
        let mut args = self.command_args.clone();
        for arg in &mut args {
            if arg == "INPUT_FILE" {
                *arg = self.input_file.clone();
            }
        }
        
        // Run the command
        let output = Command::new(binary_path)
            .args(&args)
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        
        // Read expected output if it exists
        let expected_output = if Path::new(&self.expected_output_file).exists() {
            Some(fs::read_to_string(&self.expected_output_file)?)
        } else {
            None
        };
        
        Ok(TestResult {
            test_case: self.name.clone(),
            success: output.status.success(),
            stdout,
            stderr,
            expected_output,
            exit_code: output.status.code(),
        })
    }
}

#[derive(Debug)]
pub struct TestResult {
    pub test_case: String,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub expected_output: Option<String>,
    pub exit_code: Option<i32>,
}

impl TestResult {
    pub fn matches_expected(&self) -> bool {
        if let Some(ref expected) = self.expected_output {
            self.success && self.stdout.trim() == expected.trim()
        } else {
            self.success
        }
    }
    
    pub fn print_summary(&self) {
        let status = if self.matches_expected() {
            "✅ PASS"
        } else {
            "❌ FAIL"
        };
        
        println!("{} Test {}", status, self.test_case);
        
        if !self.matches_expected() {
            if !self.success {
                println!("  Exit code: {:?}", self.exit_code);
                if !self.stderr.is_empty() {
                    println!("  Error: {}", self.stderr);
                }
            } else if let Some(ref expected) = self.expected_output {
                println!("  Expected output length: {}", expected.len());
                println!("  Actual output length: {}", self.stdout.len());
                
                // Show first difference
                let expected_lines: Vec<&str> = expected.lines().collect();
                let actual_lines: Vec<&str> = self.stdout.lines().collect();
                
                for (i, (exp, act)) in expected_lines.iter().zip(actual_lines.iter()).enumerate() {
                    if exp != act {
                        println!("  First difference at line {}: expected '{}', got '{}'", i + 1, exp, act);
                        break;
                    }
                }
            }
        }
    }
}

pub fn run_all_tests(binary_path: &Path, test_dir: &Path) -> Result<Vec<TestResult>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    
    // Find all input test files
    let entries = fs::read_dir(test_dir)?;
    let mut test_cases = Vec::new();
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if let Some(filename) = path.file_name() {
            if filename.to_string_lossy().contains("-in-") && filename.to_string_lossy().ends_with(".adi") {
                if let Some(test_case) = TestCase::from_input_file(&path) {
                    test_cases.push(test_case);
                }
            }
        }
    }
    
    // Sort test cases by name
    test_cases.sort_by(|a, b| a.name.cmp(&b.name));
    
    // Run each test case
    for test_case in test_cases {
        println!("Running test {}...", test_case.name);
        match test_case.run(binary_path) {
            Ok(result) => {
                result.print_summary();
                results.push(result);
            }
            Err(e) => {
                println!("❌ FAIL Test {} - Error running test: {}", test_case.name, e);
            }
        }
        println!();
    }
    
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_command_args() {
        let content = r#"Plain ASCII file with no encoding specified.

Command: `transadif {filename}`

<PROGRAMID:9>TransADIF"#;
        
        let args = TestCase::extract_command_args(content);
        assert_eq!(args, Some(vec!["INPUT_FILE".to_string()]));
    }
}
