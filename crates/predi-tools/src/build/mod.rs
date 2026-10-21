//! `predi-build`: corpus splits and language asset (contracts/cli.md).

pub mod lexicon;
pub mod ngrams;
pub mod pii;
pub mod sources;
pub mod splits;
pub mod tune;
pub mod writer;

use crate::util::{
    Result, ToolError, read_bytes, read_list, read_list_optional, sha256, sha256_hex,
};
use lexicon::{LexiconInputs, Text, build_lexicon, count_words};
use ngrams::{NgramParams, build_ngrams};
use pii::{Allowlist, drop_proper_names, scan_vocabulary};
use predi_core::normalize::lower;
use serde::Deserialize;
use sources::{Corpus, Split, check_leakage, read_tatoeba, read_words};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use writer::{HeaderParams, write_asset};

/// Default cutoffs until `--tune-tau` tunes them (research R6).
pub const DEFAULT_TAU_COMPLETION: f32 = 0.15;
pub const DEFAULT_TAU_NEXT: f32 = 0.15;
/// Stupid Backoff factor (research R2).
pub const DEFAULT_ALPHA: f32 = 0.4;
/// Up-weighting of team-written domain counts against general text (research R2).
pub const DEFAULT_DOMAIN_WEIGHT: u32 = 50;
/// Asset size target (research R10).
pub const MAX_ASSET_BYTES: usize = 40 * 1024 * 1024;

/// Where the build reads its text from.
#[derive(Debug, Clone)]
pub enum SourceMode {
    /// Real build: the source manifest plus the team corpus with frozen splits.
    Manifest { manifest: PathBuf, corpus: PathBuf },
    /// Test fixtures: `sentences.txt` and `words.txt` in a directory; no manifest, no splits.
    Plain { dir: PathBuf },
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub mode: SourceMode,
    pub allowlist: PathBuf,
    pub register_flags: PathBuf,
    pub domain_words: PathBuf,
    pub given_names: PathBuf,
    pub asset_version: String,
    pub tune_tau: bool,
    pub domain_weight: u32,
    pub ngram: NgramParams,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    source: Vec<ManifestSource>,
}

#[derive(Debug, Deserialize)]
struct ManifestSource {
    name: String,
    role: String,
    license: String,
    sha256: String,
    extracted: String,
}

pub struct BuildOutput {
    pub asset: Vec<u8>,
    pub report: String,
}

fn manifest_root(manifest: &Path) -> PathBuf {
    // data/sources/manifest.toml → repository root.
    manifest
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

struct Inputs {
    texts: Vec<Text>,
    dictionary: HashSet<String>,
    manifest_sha: [u8; 32],
    dev: Vec<String>,
    notes: Vec<String>,
}

fn load_manifest_inputs(manifest: &Path, corpus_dir: &Path, weight: u32) -> Result<Inputs> {
    let raw = read_bytes(manifest)?;
    let text =
        String::from_utf8(raw.clone()).map_err(|_| ToolError::invalid("manifest is not UTF-8"))?;
    let parsed: Manifest = toml::from_str(&text)
        .map_err(|e| ToolError::invalid(format!("{}: {e}", manifest.display())))?;
    let root = manifest_root(manifest);
    let by_role = |role: &str| -> Result<&ManifestSource> {
        parsed
            .source
            .iter()
            .find(|s| s.role == role)
            .ok_or_else(|| ToolError::invalid(format!("manifest has no '{role}' source")))
    };
    let general = by_role("general")?;
    let spelling = by_role("spelling")?;
    let domain = by_role("domain")?;

    let messages_path = corpus_dir.join("messages.tsv");
    let actual = sha256_hex(&read_bytes(&messages_path)?);
    if actual != domain.sha256 {
        return Err(ToolError::invalid(format!(
            "{} changed (sha256 {actual}); update the '{}' entry in the manifest and bump the asset version",
            messages_path.display(),
            domain.name
        )));
    }

    let corpus = Corpus::load(corpus_dir)?;
    let build_msgs = corpus.split(Split::Build)?;
    let dev: Vec<String> = corpus
        .split(Split::Dev)?
        .iter()
        .map(|m| m.text.clone())
        .collect();
    let mut general_sentences = read_tatoeba(&root.join(&general.extracted))?;
    let dropped = check_leakage(&mut general_sentences, &build_msgs, &corpus.test_hashes())?;

    let mut texts: Vec<Text> = general_sentences
        .into_iter()
        .map(|t| Text {
            text: t,
            weight: 1.0,
        })
        .collect();
    let n_general = texts.len();
    texts.extend(build_msgs.iter().map(|m| Text {
        text: m.text.clone(),
        weight: weight as f64,
    }));

    let notes = vec![
        format!(
            "general source: {} ({}), {} sentences",
            general.name, general.license, n_general
        ),
        format!("spelling source: {} ({})", spelling.name, spelling.license),
        format!(
            "domain source: {} ({}), {} build-split messages, weight {weight}",
            domain.name,
            domain.license,
            build_msgs.len()
        ),
        format!("dev-split messages (tuning only): {}", dev.len()),
        format!(
            "test-split leakage check: passed; {dropped} general sentences identical to test messages excluded"
        ),
    ];
    Ok(Inputs {
        texts,
        dictionary: read_words(&root.join(&spelling.extracted))?,
        manifest_sha: sha256(&raw),
        dev,
        notes,
    })
}

fn load_plain_inputs(dir: &Path) -> Result<Inputs> {
    let sentences = read_list(&dir.join("sentences.txt"))?;
    let texts = sentences
        .into_iter()
        .map(|t| Text {
            text: t,
            weight: 1.0,
        })
        .collect::<Vec<_>>();
    let notes = vec![format!(
        "plain sources (test fixture): {} sentences from {}",
        texts.len(),
        dir.display()
    )];
    Ok(Inputs {
        texts,
        dictionary: read_words(&dir.join("words.txt"))?,
        manifest_sha: [0; 32],
        dev: Vec::new(),
        notes,
    })
}

/// Build the asset and its report.
pub fn build(opts: &BuildOptions) -> Result<BuildOutput> {
    let inputs = match &opts.mode {
        SourceMode::Manifest { manifest, corpus } => {
            load_manifest_inputs(manifest, corpus, opts.domain_weight)?
        }
        SourceMode::Plain { dir } => load_plain_inputs(dir)?,
    };
    if opts.tune_tau && inputs.dev.is_empty() {
        return Err(ToolError::invalid(
            "--tune-tau needs a corpus with a dev split",
        ));
    }

    let allow = Allowlist::new(&read_list(&opts.allowlist)?);
    let register: HashSet<String> = read_list_optional(&opts.register_flags)?
        .iter()
        .map(|w| lower(w))
        .collect();
    let domain_words: HashSet<String> = read_list_optional(&opts.domain_words)?
        .iter()
        .map(|w| lower(w))
        .collect();
    let given_names = read_list_optional(&opts.given_names)?;

    // Count, filter personal data, validate spelling.
    let (mut stats, non_word) = count_words(&inputs.texts);
    let dropped_names = drop_proper_names(&mut stats, &allow);
    let lex_inputs = LexiconInputs {
        dictionary: &inputs.dictionary,
        domain_words: &domain_words,
        allowlist: &allow,
        register_flags: &register,
    };
    let (lex, rejected) = build_lexicon(&stats, &lex_inputs);
    let ngrams = build_ngrams(&inputs.texts, &lex, opts.ngram);

    // Build-time personal-data scan of the finished vocabulary (FR-028).
    let vocab: Vec<_> = lex
        .entries
        .iter()
        .map(|e| (e.canonical.clone(), e.casing))
        .collect();
    scan_vocabulary(&vocab, &given_names, &allow)?;

    let mut header = HeaderParams {
        asset_version: opts.asset_version.clone(),
        tau_completion: DEFAULT_TAU_COMPLETION,
        tau_next: DEFAULT_TAU_NEXT,
        alpha: DEFAULT_ALPHA,
        manifest_sha256: inputs.manifest_sha,
    };
    let (mut asset, sizes) = write_asset(&header, &lex, &ngrams)?;

    let mut tuning_notes = Vec::new();
    if opts.tune_tau {
        let tuned = tune::tune(&asset, &inputs.dev)?;
        header.tau_completion = tuned.tau_completion;
        header.tau_next = tuned.tau_next;
        writer::set_tau(&mut asset, tuned.tau_completion, tuned.tau_next);
        tuning_notes = tuned.notes;
    }

    // The asset must load in the engine exactly as written.
    predi_core::Engine::from_bytes(&asset)
        .map_err(|e| ToolError::invalid(format!("written asset does not load: {e}")))?;
    if asset.len() > MAX_ASSET_BYTES {
        return Err(ToolError::invalid(format!(
            "asset is {} bytes, above the {MAX_ASSET_BYTES}-byte target; raise the pruning thresholds",
            asset.len()
        )));
    }

    let mut r = String::new();
    let _ = writeln!(r, "PrediType language asset build report");
    let _ = writeln!(r, "asset version: {}", header.asset_version);
    let _ = writeln!(
        r,
        "format version: {}",
        predi_core::asset::layout::FORMAT_VERSION
    );
    let _ = writeln!(
        r,
        "source manifest sha256: {}",
        crate::util::hex(&inputs.manifest_sha)
    );
    let _ = writeln!(r, "payload sha256: {}", sha256_hex(&asset[128..]));
    for n in &inputs.notes {
        let _ = writeln!(r, "{n}");
    }
    let _ = writeln!(r);
    let _ = writeln!(
        r,
        "size: {} bytes ({:.2} MB, target <= 40 MB)",
        sizes.total,
        sizes.total as f64 / 1_048_576.0
    );
    let names = [
        "lexicon FST",
        "spellings",
        "word flags",
        "n-gram tables",
        "top-next lists",
        "reserved",
    ];
    for (name, s) in names.iter().zip(sizes.sections) {
        let _ = writeln!(r, "  {name}: {s} bytes");
    }
    let _ = writeln!(r);
    let _ = writeln!(r, "vocabulary: {} words", lex.entries.len());
    let _ = writeln!(
        r,
        "  register-flagged (FR-022): {}",
        lex.entries.iter().filter(|e| e.register_other).count()
    );
    let _ = writeln!(
        r,
        "  allowlisted capitalized/acronym: {}",
        lex.entries
            .iter()
            .filter(|e| e.casing != predi_core::casing::LexCasing::Lower)
            .count()
    );
    let _ = writeln!(r, "tokens counted (weighted): {:.0}", ngrams.stats.tokens);
    let _ = writeln!(
        r,
        "pruning: bigram min count {}, trigram min count {}",
        opts.ngram.min_bigram, opts.ngram.min_trigram
    );
    let _ = writeln!(
        r,
        "  bigrams: {} seen, {} kept",
        ngrams.stats.bigrams_seen, ngrams.stats.bigrams_kept
    );
    let _ = writeln!(
        r,
        "  trigrams: {} seen, {} kept",
        ngrams.stats.trigrams_seen, ngrams.stats.trigrams_kept
    );
    let _ = writeln!(
        r,
        "  two-word contexts with next-word lists: {}",
        ngrams.bictx_keys.len()
    );
    let _ = writeln!(r);
    let _ = writeln!(r, "tau_completion: {}", header.tau_completion);
    let _ = writeln!(r, "tau_next: {}", header.tau_next);
    let _ = writeln!(r, "alpha: {}", header.alpha);
    let _ = writeln!(
        r,
        "tau tuned on dev split: {}",
        if opts.tune_tau {
            "yes"
        } else {
            "no (defaults)"
        }
    );
    for n in &tuning_notes {
        let _ = writeln!(r, "  {n}");
    }
    let _ = writeln!(r);
    let _ = writeln!(r, "personal-data filter (research R8):");
    let _ = writeln!(
        r,
        "  non-word tokens dropped (placeholders, numbers, emails, URLs, code): {non_word}"
    );
    let _ = writeln!(
        r,
        "  likely proper names dropped: {} distinct",
        dropped_names.len()
    );
    for (w, c) in dropped_names.iter().take(100) {
        let _ = writeln!(r, "    {w}\t{c}");
    }
    let _ = writeln!(r, "  build-time vocabulary scan: passed (0 matches)");
    let _ = writeln!(r);
    let _ = writeln!(
        r,
        "spelling validation: {} distinct non-dictionary words excluded (top 200 by count)",
        rejected.len()
    );
    for (w, c) in rejected.iter().take(200) {
        let _ = writeln!(r, "    {w}\t{c}");
    }
    Ok(BuildOutput { asset, report: r })
}
