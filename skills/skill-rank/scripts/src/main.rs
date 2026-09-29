use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let env = match skill_rank::cli::Env::from_process() {
        Ok(env) => env,
        Err(error) => {
            eprintln!("skill-rank: {error}");
            return ExitCode::from(1);
        }
    };
    match skill_rank::cli::run(&args, &env) {
        Ok(outcome) if outcome.code == 0 => {
            print!("{}", outcome.text);
            ExitCode::SUCCESS
        }
        Ok(outcome) => {
            eprint!("{}", outcome.text);
            ExitCode::from(outcome.code)
        }
        Err(error) => {
            eprintln!("skill-rank: {error}");
            ExitCode::from(1)
        }
    }
}
