use std::env;
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 3 {
        eprintln!("Usage: {} <binary_path> <test_directory>", args[0]);
        process::exit(1);
    }
    
    let binary_path = Path::new(&args[1]);
    let test_dir = Path::new(&args[2]);
    
    if !binary_path.exists() {
        eprintln!("Binary not found: {}", binary_path.display());
        process::exit(1);
    }
    
    if !test_dir.exists() {
        eprintln!("Test directory not found: {}", test_dir.display());
        process::exit(1);
    }
    
    println!("Running tests with binary: {}", binary_path.display());
    println!("Test directory: {}", test_dir.display());
    println!();
    
    match transadif::test_runner::run_all_tests(binary_path, test_dir) {
        Ok(results) => {
            let total = results.len();
            let passed = results.iter().filter(|r| r.matches_expected()).count();
            let failed = total - passed;
            
            println!("=== Test Summary ===");
            println!("Total tests: {}", total);
            println!("Passed: {}", passed);
            println!("Failed: {}", failed);
            
            if failed > 0 {
                process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error running tests: {}", e);
            process::exit(1);
        }
    }
}
