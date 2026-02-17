mod assistant_case;
mod case;
mod cli;
mod engine;
mod fixture_io;
mod quality;

use cli::{CliError, CliOptions};
use engine::run_eval;

#[tokio::main]
async fn main() {
    let options = match CliOptions::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(CliError::HelpRequested) => {
            print_usage();
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("error: {err}");
            eprintln!();
            print_usage();
            std::process::exit(2);
        }
    };

    match run_eval(&options).await {
        Ok(summary) => {
            summary.print();
            if summary.has_failures() {
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("failed to run llm eval harness: {err}");
            std::process::exit(2);
        }
    }
}

fn print_usage() {
    eprintln!(
        "Usage: cargo run -p llm-eval -- [--mode mocked|live] [--update-goldens]\n\
         \n\
         Modes:\n\
         - mocked (default): deterministic fixture-based checks + golden comparison\n\
         - live: optional OpenRouter smoke mode (no golden comparison)\n\
         \n\
         Options:\n\
         - --update-goldens  Rewrite mocked-mode goldens intentionally\n\
         - --help            Show this help text"
    );
}
