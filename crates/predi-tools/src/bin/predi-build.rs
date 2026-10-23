//! `predi-build`: assign corpus splits and build the language asset (contracts/cli.md).

use clap::{Args, Parser, Subcommand};
use predi_tools::build::{self, BuildOptions, SourceMode, ngrams::NgramParams, splits};
use predi_tools::util::{ToolError, write_bytes};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "predi-build",
    about = "Build PrediType corpus splits and language assets"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Assign test/dev/build splits by SHA-256 rank and write splits.tsv (run once).
    Split {
        #[arg(long, default_value = "data/corpus/es-co-cs")]
        corpus: PathBuf,
    },
    /// Build the language asset and its build report.
    Build(Box<BuildArgs>),
}

#[derive(Args)]
struct BuildArgs {
    #[arg(
        long,
        default_value = "data/sources/manifest.toml",
        conflicts_with = "plain_sources"
    )]
    manifest: PathBuf,
    #[arg(
        long,
        default_value = "data/corpus/es-co-cs",
        conflicts_with = "plain_sources"
    )]
    corpus: PathBuf,
    /// Test fixtures only: read sentences.txt and words.txt from this directory.
    #[arg(long)]
    plain_sources: Option<PathBuf>,
    #[arg(long, default_value = "data/allowlist-proper-nouns.txt")]
    allowlist: PathBuf,
    #[arg(long, default_value = "data/lexicon/register-flags.txt")]
    register_flags: PathBuf,
    #[arg(long, default_value = "data/lexicon/domain-words.txt")]
    domain_words: PathBuf,
    #[arg(long, default_value = "data/given-names.txt")]
    given_names: PathBuf,
    #[arg(long)]
    asset_version: String,
    #[arg(long)]
    out: PathBuf,
    /// Tune tau on the dev split (research R6).
    #[arg(long)]
    tune_tau: bool,
    /// Weight of team-written domain counts relative to general text.
    #[arg(long, default_value_t = build::DEFAULT_DOMAIN_WEIGHT)]
    domain_weight: u32,
    #[arg(long, default_value_t = 2.0)]
    min_bigram: f64,
    #[arg(long, default_value_t = 2.0)]
    min_trigram: f64,
}

fn report_path(out: &std::path::Path) -> PathBuf {
    let stem = out
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    out.with_file_name(format!("{stem}.build-report.txt"))
}

fn run(cli: Cli) -> Result<(), ToolError> {
    match cli.command {
        Command::Split { corpus } => {
            let s = splits::assign(&corpus)?;
            println!(
                "assigned {} messages: test {}, dev {}, build {} ({} test word positions, {} newly assigned)",
                s.messages, s.test, s.dev, s.build, s.test_words, s.newly_assigned
            );
            println!("wrote {}", corpus.join("splits.tsv").display());
        }
        Command::Build(a) => {
            let mode = match a.plain_sources {
                Some(dir) => SourceMode::Plain { dir },
                None => SourceMode::Manifest {
                    manifest: a.manifest,
                    corpus: a.corpus,
                },
            };
            let opts = BuildOptions {
                mode,
                allowlist: a.allowlist,
                register_flags: a.register_flags,
                domain_words: a.domain_words,
                given_names: a.given_names,
                asset_version: a.asset_version,
                tune_tau: a.tune_tau,
                domain_weight: a.domain_weight,
                ngram: NgramParams {
                    min_bigram: a.min_bigram,
                    min_trigram: a.min_trigram,
                },
            };
            let out = build::build(&opts)?;
            write_bytes(&a.out, &out.asset)?;
            let report = report_path(&a.out);
            write_bytes(&report, out.report.as_bytes())?;
            println!("wrote {} ({} bytes)", a.out.display(), out.asset.len());
            println!("wrote {}", report.display());
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("predi-build: {e}");
            ExitCode::from(e.code as u8)
        }
    }
}
