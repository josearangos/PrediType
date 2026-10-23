//! `predi-eval`: repeatable evaluation (contracts/cli.md). Writes only the report file.

use clap::{Parser, ValueEnum};
use predi_tools::build::sources::Split;
use predi_tools::eval::{self, EvalOptions, Only};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

#[derive(Clone, Copy, ValueEnum)]
enum SplitArg {
    Test,
    Dev,
}

#[derive(Clone, Copy, ValueEnum)]
enum OnlyArg {
    Quality,
    Correctness,
    Performance,
}

#[derive(Parser)]
#[command(
    name = "predi-eval",
    about = "Evaluate a PrediType language asset against the success criteria"
)]
struct Cli {
    #[arg(long, default_value = "assets/es-CO-1.1.0.pta")]
    asset: PathBuf,
    #[arg(long, default_value = "data/corpus/es-co-cs")]
    corpus: PathBuf,
    #[arg(long, value_enum, default_value = "test")]
    split: SplitArg,
    #[arg(long, default_value = "data/correctness/cases.jsonl")]
    cases: PathBuf,
    #[arg(long, default_value = "docs/reference-computer.toml")]
    machine: PathBuf,
    #[arg(long, default_value = "reports/")]
    out: PathBuf,
    #[arg(long, default_value = "data/sources/raw/es_CO.words")]
    words: PathBuf,
    #[arg(long, default_value = "data/lexicon/domain-words.txt")]
    domain_words: PathBuf,
    #[arg(long, default_value = "data/allowlist-proper-nouns.txt")]
    allowlist: PathBuf,
    #[arg(long, default_value = "data/given-names.txt")]
    given_names: PathBuf,
    /// Replay the whole split twice and compare every result (SC-008).
    #[arg(long)]
    check_determinism: bool,
    #[arg(long, value_enum)]
    only: Option<OnlyArg>,
}

fn main() -> ExitCode {
    let start = Instant::now();
    let cli = Cli::parse();
    let opts = EvalOptions {
        asset: cli.asset,
        corpus: cli.corpus,
        split: match cli.split {
            SplitArg::Test => Split::Test,
            SplitArg::Dev => Split::Dev,
        },
        cases: cli.cases,
        machine: cli.machine,
        words: cli.words,
        domain_words: cli.domain_words,
        allowlist: cli.allowlist,
        given_names: cli.given_names,
        check_determinism: cli.check_determinism,
        only: cli.only.map(|o| match o {
            OnlyArg::Quality => Only::Quality,
            OnlyArg::Correctness => Only::Correctness,
            OnlyArg::Performance => Only::Performance,
        }),
    };
    let report = match eval::run(&opts, start) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("predi-eval: {e}");
            return ExitCode::from(e.code as u8);
        }
    };
    match report.write(&cli.out) {
        Ok(path) => println!("wrote {}", path.display()),
        Err(e) => {
            eprintln!("predi-eval: {e}");
            return ExitCode::from(2);
        }
    }
    print!("{}", report.table());
    println!(
        "SC-012 (privacy) is verified outside this process: scripts/check-privacy.sh or Process Monitor."
    );
    if report.passed_hard_gates {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
