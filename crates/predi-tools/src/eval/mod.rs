//! `predi-eval`: repeatable evaluation (contracts/cli.md, research R11, FR-037).

pub mod checks;
pub mod json;
pub mod metrics;
pub mod perf;
pub mod replay;
pub mod report;

use crate::build::pii::{Allowlist, find_personal_data};
use crate::build::sources::{Corpus, Split};
use crate::util::{
    Result, ToolError, read_bytes, read_list, read_list_optional, read_to_string, sha256_hex,
};
use metrics::{QualityCounts, SpellingReference, ratio, simulate_savings};
use predi_core::asset::Asset;
use predi_core::normalize::lower;
use predi_core::{Engine, PredictionRequest};
use report::{Comparison, Criterion, Detail, Machine, Report};
use std::path::PathBuf;
use std::time::Instant;

/// Which criteria to evaluate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Only {
    Quality,
    Correctness,
    Performance,
}

#[derive(Debug, Clone)]
pub struct EvalOptions {
    pub asset: PathBuf,
    pub corpus: PathBuf,
    pub split: Split,
    pub cases: PathBuf,
    pub machine: PathBuf,
    pub words: PathBuf,
    pub domain_words: PathBuf,
    pub allowlist: PathBuf,
    pub given_names: PathBuf,
    pub check_determinism: bool,
    pub only: Option<Only>,
}

/// Requests sampled for SC-013 and (without `--check-determinism`) SC-008.
pub const SAMPLE_REQUESTS: usize = 1000;

fn sample<'a>(inputs: &[&'a str], n: usize) -> Vec<&'a str> {
    if inputs.len() <= n {
        return inputs.to_vec();
    }
    (0..n).map(|i| inputs[i * inputs.len() / n]).collect()
}

/// Run the evaluation. `process_start` is taken at the top of `main` (SC-011).
pub fn run(opts: &EvalOptions, process_start: Instant) -> Result<Report> {
    let want = |o: Only| opts.only.is_none_or(|x| x == o);
    let mut memory = perf::MemoryPeak::default();

    // Startup: process start → first successful prediction.
    let asset_bytes = read_bytes(&opts.asset)?;
    let engine = Engine::from_bytes(&asset_bytes)
        .map_err(|e| ToolError::invalid(format!("{}: {e}", opts.asset.display())))?;
    let _ = engine.predict(&PredictionRequest::new(0, ""));
    let startup_s = process_start.elapsed().as_secs_f64();

    let corpus = Corpus::load(&opts.corpus)?;
    let messages: Vec<String> = corpus
        .split(opts.split)?
        .iter()
        .map(|m| m.text.clone())
        .collect();
    let corpus_sha = {
        let mut bytes = read_bytes(&opts.corpus.join("messages.tsv"))?;
        bytes.extend(read_bytes(&opts.corpus.join("splits.tsv"))?);
        sha256_hex(&bytes)
    };
    let machine = Machine::load(&opts.machine)?;

    let mut criteria = Vec::new();
    // Every replay input, for performance and determinism.
    let inputs: Vec<&str> = messages
        .iter()
        .flat_map(|m| replay::points(m).into_iter().map(move |p| &m[..p.pos]))
        .collect();

    // Performance first, before the evaluator loads its own large reference data (the spelling
    // dictionary), so the working-set peak reflects the engine replay (SC-010).
    let mut latency = None;
    if want(Only::Performance) {
        let l = perf::measure(&engine, &inputs, &mut memory);
        let mut c = Criterion::new(
            "SC-009",
            "p95 latency per request (p99 must also be <= 200 ms)",
            l.p95,
            Some("ms"),
            100.0,
            Comparison::AtMost,
        )
        .detail("p99_ms", Detail::Number(l.p99))
        .detail("p99_threshold_ms", Detail::Number(200.0))
        .detail("requests", Detail::Number(l.requests as f64));
        if l.p99 > 200.0 {
            c = c.force_fail();
        }
        criteria.push(c);
        let peak_mb = memory.peak_bytes as f64 / 1_048_576.0;
        let mut c = Criterion::new(
            "SC-010",
            "Peak working set of the evaluation process during the replay",
            peak_mb,
            Some("MB"),
            100.0,
            Comparison::AtMost,
        )
        .detail("samples", Detail::Number(memory.samples as f64));
        if memory.samples == 0 {
            c = c.force_fail();
        }
        criteria.push(c);
        criteria.push(Criterion::new(
            "SC-011",
            "Time from process start to the first prediction",
            startup_s,
            Some("s"),
            3.0,
            Comparison::AtMost,
        ));
        latency = Some(l);
    }

    if want(Only::Quality) || want(Only::Correctness) {
        let spelling = SpellingReference {
            words: read_list(&opts.words)?.into_iter().collect(),
            domain: read_list_optional(&opts.domain_words)?
                .iter()
                .map(|w| lower(w))
                .collect(),
            allowlist: read_list(&opts.allowlist)?.into_iter().collect(),
        };
        let mut q = QualityCounts::default();
        let mut id = 0u64;
        let (mut chars, mut typed, mut accepted) = (0u64, 0u64, 0u64);
        for m in &messages {
            let replayed = replay::replay_message(&engine, m, &mut id);
            q.add_message(m, &replayed, &spelling);
            if want(Only::Quality) {
                let (c, t, a) = simulate_savings(&engine, m, &mut id);
                chars += c;
                typed += t;
                accepted += a;
            }
        }

        if want(Only::Quality) {
            let asset =
                Asset::from_bytes(&asset_bytes).map_err(|e| ToolError::invalid(e.to_string()))?;
            let vocab: Vec<_> = (0..asset.n_words())
                .map(|id| (asset.spelling(id).to_string(), asset.casing(id)))
                .collect();
            let allow = Allowlist::new(&read_list(&opts.allowlist)?);
            let scan = find_personal_data(&vocab, &read_list_optional(&opts.given_names)?, &allow);

            criteria.push(
                Criterion::new(
                    "SC-001",
                    "Partial-word points: intended word among the suggestions",
                    ratio(q.partial_hits, q.partial_points),
                    None,
                    0.70,
                    Comparison::AtLeast,
                )
                .detail("points", Detail::Number(q.partial_points as f64)),
            );
            criteria.push(
                Criterion::new(
                    "SC-002",
                    "Boundary points: actual next word among the suggestions",
                    ratio(q.boundary_hits, q.boundary_points),
                    None,
                    0.35,
                    Comparison::AtLeast,
                )
                .detail("points", Detail::Number(q.boundary_points as f64)),
            );
            let saved = chars.saturating_sub(typed).saturating_sub(accepted);
            criteria.push(
                Criterion::new(
                    "SC-003",
                    "Simulated keystroke savings",
                    ratio(saved, chars),
                    None,
                    0.20,
                    Comparison::AtLeast,
                )
                .detail("characters", Detail::Number(chars as f64))
                .detail("keystrokes_typed", Detail::Number(typed as f64))
                .detail("acceptances", Detail::Number(accepted as f64)),
            );
            criteria.push(
                Criterion::new(
                    "SC-004",
                    "First rank when a correct suggestion is present",
                    ratio(q.correct_first, q.correct_present),
                    None,
                    0.60,
                    Comparison::AtLeast,
                )
                .detail(
                    "points_with_correct",
                    Detail::Number(q.correct_present as f64),
                ),
            );
            criteria.push(
                Criterion::new(
                    "SC-005",
                    "Suggestions with a misspelling or personal data, plus asset scan matches",
                    (q.misspelled_or_personal + scan.len() as u64) as f64,
                    None,
                    0.0,
                    Comparison::Equal,
                )
                .detail(
                    "suggestions_checked",
                    Detail::Number(q.suggestions_checked as f64),
                )
                .detail(
                    "suggestion_violations",
                    Detail::Number(q.misspelled_or_personal as f64),
                )
                .detail("asset_scan_matches", Detail::Number(scan.len() as f64)),
            );
            criteria.push(
                Criterion::new(
                    "SC-014",
                    "First-rank precision among non-empty results",
                    ratio(q.nonempty_first_correct, q.nonempty),
                    None,
                    0.50,
                    Comparison::AtLeast,
                )
                .detail("nonempty_points", Detail::Number(q.nonempty as f64)),
            );
            criteria.push(
                Criterion::new(
                    "SC-015",
                    "Continuations shown that match the following text in full",
                    ratio(q.continuations_full_match, q.continuations_shown),
                    None,
                    0.40,
                    Comparison::AtLeast,
                )
                .detail(
                    "continuations_shown",
                    Detail::Number(q.continuations_shown as f64),
                ),
            );
        }

        if want(Only::Correctness) {
            let (passed, total, failing) =
                checks::correctness(&engine, &read_to_string(&opts.cases)?)?;
            criteria.push(
                Criterion::new(
                    "SC-006",
                    "Correctness test set cases passing",
                    ratio(passed as u64, total as u64),
                    None,
                    1.0,
                    Comparison::Equal,
                )
                .detail("cases", Detail::Number(total as f64))
                .detail("failing", Detail::CaseIds(failing)),
            );
            criteria.push(
                Criterion::new(
                    "SC-007",
                    "Applied suggestions that leave the text before the partial word unchanged",
                    if q.applied == 0 {
                        1.0
                    } else {
                        ratio(q.applied - q.prefix_violations, q.applied)
                    },
                    None,
                    1.0,
                    Comparison::Equal,
                )
                .detail("applied", Detail::Number(q.applied as f64))
                .detail("violations", Detail::Number(q.prefix_violations as f64)),
            );
            let det_inputs = if opts.check_determinism {
                inputs.clone()
            } else {
                sample(&inputs, SAMPLE_REQUESTS)
            };
            let diffs = checks::determinism(&asset_bytes, &det_inputs)?;
            criteria.push(
                Criterion::new(
                    "SC-008",
                    "Two runs give identical results (1 = identical)",
                    if diffs == 0 { 1.0 } else { 0.0 },
                    None,
                    1.0,
                    Comparison::Equal,
                )
                .detail("requests_compared", Detail::Number(det_inputs.len() as f64))
                .detail("differences", Detail::Number(diffs as f64))
                .detail("full_replay", Detail::Flag(opts.check_determinism)),
            );
            let fc_inputs = sample(&inputs, SAMPLE_REQUESTS);
            let (equal, compared) = checks::field_change(&engine, &asset_bytes, &fc_inputs)?;
            criteria.push(
                Criterion::new(
                    "SC-013",
                    "Results after a field change equal a fresh engine's",
                    ratio(equal, compared),
                    None,
                    1.0,
                    Comparison::Equal,
                )
                .detail("requests_compared", Detail::Number(compared as f64)),
            );
        }
    }

    criteria.sort_by_key(|c| c.id);
    let mut report = Report {
        generated_at: report::utc_now(),
        engine_version: predi_core::ENGINE_VERSION.to_string(),
        asset_version: engine.info().asset_version,
        corpus_manifest_sha256: corpus_sha,
        machine,
        criteria,
        latency_ms: latency,
        passed_hard_gates: false,
    };
    report.compute_gates();
    Ok(report)
}
