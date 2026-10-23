//! `predi-cli`: manual prediction (contracts/cli.md). Writes nothing to disk, keeps no history.

use clap::{Parser, Subcommand};
use predi_core::{Engine, PredictionRequest};
use predi_tools::eval::json;
use predi_tools::util::read_bytes;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "predi-cli", about = "Try the PrediType engine by hand")]
struct Cli {
    #[arg(long, default_value = "assets/es-CO-1.1.0.pta")]
    asset: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Predict for one text and print the result as JSON.
    Predict {
        /// The text before the caret (may be empty).
        text: String,
    },
    /// Read one text per line from stdin and print each result as JSON.
    Repl,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let engine = match read_bytes(&cli.asset)
        .map_err(|e| e.message)
        .and_then(|b| Engine::from_bytes(&b).map_err(|e| format!("{}: {e}", cli.asset.display())))
    {
        Ok(e) => e,
        Err(e) => {
            eprintln!("predi-cli: {e}");
            return ExitCode::from(2);
        }
    };
    match cli.command {
        Command::Predict { text } => {
            println!(
                "{}",
                json::to_string(&engine.predict(&PredictionRequest::new(1, &text)))
            );
        }
        Command::Repl => {
            let stdin = std::io::stdin();
            let mut stdout = std::io::stdout();
            let mut id = 0u64;
            for line in stdin.lock().lines() {
                let Ok(line) = line else { break };
                id += 1;
                let r = engine.predict(&PredictionRequest::new(id, &line));
                let _ = writeln!(stdout, "{}", json::to_string(&r));
                let _ = stdout.flush();
            }
        }
    }
    ExitCode::SUCCESS
}
