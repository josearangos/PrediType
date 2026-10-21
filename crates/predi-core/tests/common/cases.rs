//! Correctness-case semantics (contracts/correctness-case.schema.json), shared by the predi-core
//! correctness test (fixture asset, native and WASM) and `predi-eval` (real asset, SC-006).
//!
//! - `contains`: each listed text equals some suggestion's text exactly.
//! - `not_contains`: no suggestion's text equals a listed text, ignoring case.
//! - `all_*`, `leading_space`, `replace_chars`, `max_words`: hold for every suggestion (vacuously
//!   true for an empty result; add `"empty": false` to require suggestions).

use predi_core::{PredictionResult, Status};
use serde_json::Value;

/// One parsed case.
pub struct Case {
    pub id: String,
    pub input: String,
    pub expect: Value,
}

/// Parse `cases.jsonl`. Errors name the line number only.
pub fn parse_cases(jsonl: &str) -> Result<Vec<Case>, String> {
    let mut out = Vec::new();
    for (n, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Value =
            serde_json::from_str(line).map_err(|e| format!("cases line {}: {e}", n + 1))?;
        let get = |k: &str| {
            v.get(k)
                .ok_or_else(|| format!("cases line {}: missing {k}", n + 1))
        };
        let id = get("id")?.as_str().unwrap_or_default().to_string();
        let input = get("input")?.as_str().unwrap_or_default().to_string();
        let expect = get("expect")?.clone();
        if id.is_empty() || !expect.is_object() {
            return Err(format!("cases line {}: bad id or expect", n + 1));
        }
        out.push(Case { id, input, expect });
    }
    Ok(out)
}

fn status_str(s: Status) -> &'static str {
    s.as_str()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Check one result against a case's `expect`. Returns the names of the failed expectations.
pub fn check(case: &Case, r: &PredictionResult) -> Vec<String> {
    let mut failed = Vec::new();
    let texts: Vec<&str> = r.suggestions.iter().map(|s| s.text.as_str()).collect();
    let Some(expect) = case.expect.as_object() else {
        return vec!["expect".into()];
    };
    for (key, val) in expect {
        let ok = match key.as_str() {
            "contains" => strings(val).iter().all(|w| texts.contains(&w.as_str())),
            "not_contains" => strings(val)
                .iter()
                .all(|w| !texts.iter().any(|t| t.to_lowercase() == w.to_lowercase())),
            "empty" => val.as_bool() == Some(r.suggestions.is_empty()),
            "status" => val.as_str() == Some(status_str(r.status)),
            "all_capitalized" => {
                val.as_bool() != Some(true)
                    || texts
                        .iter()
                        .all(|t| t.chars().next().is_some_and(char::is_uppercase))
            }
            "all_lowercase" => {
                val.as_bool() != Some(true)
                    || texts.iter().all(|t| !t.chars().any(char::is_uppercase))
            }
            "all_uppercase" => {
                val.as_bool() != Some(true)
                    || texts.iter().all(|t| !t.chars().any(char::is_lowercase))
            }
            "leading_space" => r
                .suggestions
                .iter()
                .all(|s| Some(s.leading_space) == val.as_bool()),
            "replace_chars" => r
                .suggestions
                .iter()
                .all(|s| Some(s.replace_chars as u64) == val.as_u64()),
            "max_words" => r.suggestions.iter().all(|s| {
                val.as_u64()
                    .is_some_and(|m| s.text.split(' ').count() as u64 <= m)
            }),
            _ => false,
        };
        if !ok {
            failed.push(key.clone());
        }
    }
    failed
}

/// A stable textual form of a result, for native-vs-WASM digests.
pub fn canonical(r: &PredictionResult) -> String {
    let mut s = format!("{}|{}", r.request_id, status_str(r.status));
    for x in &r.suggestions {
        s.push_str(&format!(
            "|{}:{:?}:{}:{}:{}:{}:{:08x}",
            x.text,
            x.kind,
            x.replace_chars,
            x.replace_utf16,
            x.leading_space,
            x.rank,
            x.confidence.to_bits()
        ));
    }
    s
}
