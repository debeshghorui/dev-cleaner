mod app;
mod clean;
mod safety;
mod scan;
mod size;
mod targets;
mod ui;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "devclean",
    version,
    about = "Find regenerable developer caches and move them to the Trash"
)]
struct Cli {
    /// Project directory to scan. Defaults to the current directory.
    path: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let raw = match cli.path {
        Some(path) => path,
        None => match std::env::current_dir() {
            Ok(path) => path,
            Err(err) => {
                eprintln!("devclean: cannot read the current directory: {err}");
                return ExitCode::from(2);
            }
        },
    };

    let root = match safety::validate_root(&raw) {
        Ok(root) => root,
        Err(err) => {
            eprintln!("devclean: {err}");
            return ExitCode::from(2);
        }
    };

    if !std::io::stdout().is_terminal() {
        eprintln!("devclean: an interactive terminal is required");
        return ExitCode::from(2);
    }

    match app::run(root) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("devclean: {err}");
            ExitCode::from(1)
        }
    }
}
