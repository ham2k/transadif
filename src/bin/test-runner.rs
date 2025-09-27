use clap::Parser;
use std::path::PathBuf;
use transadif::test_runner::TestRunner;

#[derive(Parser)]
#[command(name = "test-runner")]
#[command(about = "Test runner for transadif")]
pub struct TestRunnerCli {
    /// Path to the test cases directory
    #[arg(default_value = "test-cases")]
    pub test_dir: PathBuf,

    /// Filter to run only specific tests
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Path to the transadif executable
    #[arg(short, long, default_value = "target/debug/transadif")]
    pub executable: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = TestRunnerCli::parse();

    let runner = TestRunner::new(args.executable);
    runner.run_all_tests(&args.test_dir, args.filter.as_deref())?;

    Ok(())
}