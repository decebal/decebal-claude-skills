use std::process::ExitCode;

fn main() -> ExitCode {
    match claude_seo::run_cli(std::env::args().skip(1)) {
        Ok(outcome) => {
            println!("{}", outcome.output);
            ExitCode::from(outcome.exit_code)
        }
        Err(error) => {
            eprintln!("claude-seo: {error}");
            ExitCode::from(1)
        }
    }
}
