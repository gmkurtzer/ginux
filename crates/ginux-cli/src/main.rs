use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = ginux_cli::Cli::parse();
    if let Err(error) = ginux_cli::validate_cli(&cli) {
        ginux_cli::print_error(&error);
        return ExitCode::from(2);
    }
    match ginux_cli::run(cli) {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(error) => {
            ginux_cli::print_error(&error);
            ExitCode::from(1)
        }
    }
}
