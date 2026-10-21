//! Evaluation report types and JSON writer (T052, contracts/eval-report.schema.json, FR-037).
//!
//! The report holds only metrics, versions, hardware details and failing case ids, never corpus,
//! typed or suggestion text.

use crate::util::{Result, ToolError, read_to_string, write_bytes};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Criteria that stay provisional until the one-time amendment (spec Success Criteria, T066).
pub const PROVISIONAL: &[&str] = &["SC-001", "SC-002", "SC-003", "SC-004", "SC-014", "SC-015"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Machine {
    pub model: String,
    pub cpu: String,
    pub cores: u32,
    pub base_clock_ghz: f64,
    pub ram_gb: f64,
    pub os_build: String,
    pub power_plan: String,
    #[serde(default)]
    pub is_reference: bool,
}

impl Machine {
    pub fn load(path: &Path) -> Result<Machine> {
        toml::from_str(&read_to_string(path)?)
            .map_err(|e| ToolError::invalid(format!("{}: {e}", path.display())))
    }
}

/// A `detail` value: a number, a flag, or a list of failing case ids.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Detail {
    Number(f64),
    Flag(bool),
    CaseIds(Vec<String>),
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum Comparison {
    #[serde(rename = ">=")]
    AtLeast,
    #[serde(rename = "<=")]
    AtMost,
    #[serde(rename = "==")]
    Equal,
}

#[derive(Debug, Clone, Serialize)]
pub struct Criterion {
    pub id: &'static str,
    pub description: &'static str,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    pub threshold: f64,
    pub comparison: Comparison,
    pub provisional: bool,
    pub passed: bool,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub detail: BTreeMap<String, Detail>,
}

impl Criterion {
    pub fn new(
        id: &'static str,
        description: &'static str,
        value: f64,
        unit: Option<&'static str>,
        threshold: f64,
        comparison: Comparison,
    ) -> Criterion {
        let passed = value.is_finite()
            && match comparison {
                Comparison::AtLeast => value >= threshold,
                Comparison::AtMost => value <= threshold,
                Comparison::Equal => value == threshold,
            };
        Criterion {
            id,
            description,
            value,
            unit,
            threshold,
            comparison,
            provisional: PROVISIONAL.contains(&id),
            passed,
            detail: BTreeMap::new(),
        }
    }

    pub fn detail(mut self, key: &str, value: Detail) -> Criterion {
        self.detail.insert(key.to_string(), value);
        self
    }

    /// Fail a criterion regardless of its value (for example when part of it was not measured).
    pub fn force_fail(mut self) -> Criterion {
        self.passed = false;
        self
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Latency {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
    pub requests: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub generated_at: String,
    pub engine_version: String,
    pub asset_version: String,
    pub corpus_manifest_sha256: String,
    pub machine: Machine,
    pub criteria: Vec<Criterion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<Latency>,
    pub passed_hard_gates: bool,
}

impl Report {
    /// Hard gates are all non-provisional criteria. Provisional criteria never fail the run.
    pub fn compute_gates(&mut self) {
        self.passed_hard_gates = self
            .criteria
            .iter()
            .filter(|c| !c.provisional)
            .all(|c| c.passed);
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Write `reports/eval-<UTC timestamp>.json` into `dir`.
    pub fn write(&self, dir: &Path) -> Result<PathBuf> {
        let stamp = self.generated_at.replace([':', '-'], "");
        let path = dir.join(format!("eval-{stamp}.json"));
        write_bytes(&path, self.to_json().as_bytes())?;
        Ok(path)
    }

    /// The pass/fail table printed after a run.
    pub fn table(&self) -> String {
        let mut s = String::from("criterion  result   provisional  value / threshold\n");
        for c in &self.criteria {
            let op = match c.comparison {
                Comparison::AtLeast => ">=",
                Comparison::AtMost => "<=",
                Comparison::Equal => "==",
            };
            s.push_str(&format!(
                "{:<10} {:<8} {:<12} {:.4}{} {op} {}{}\n",
                c.id,
                if c.passed { "PASS" } else { "FAIL" },
                if c.provisional { "yes" } else { "no" },
                c.value,
                c.unit.map(|u| format!(" {u}")).unwrap_or_default(),
                c.threshold,
                c.unit.map(|u| format!(" {u}")).unwrap_or_default(),
            ));
        }
        s.push_str(&format!(
            "hard gates: {}\n",
            if self.passed_hard_gates {
                "PASS"
            } else {
                "FAIL"
            }
        ));
        s
    }
}

/// RFC 3339 UTC timestamp from the system clock, without external crates.
pub fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_utc(secs)
}

pub fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_792_108_800), "2026-10-16T00:00:00Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn provisional_never_gates() {
        let m = Machine {
            model: "x".into(),
            cpu: "x".into(),
            cores: 4,
            base_clock_ghz: 2.0,
            ram_gb: 8.0,
            os_build: "x".into(),
            power_plan: "x".into(),
            is_reference: false,
        };
        let mut r = Report {
            generated_at: utc_now(),
            engine_version: "0".into(),
            asset_version: "0".into(),
            corpus_manifest_sha256: "0".repeat(64),
            machine: m,
            criteria: vec![
                Criterion::new("SC-001", "d", 0.1, None, 0.7, Comparison::AtLeast),
                Criterion::new("SC-006", "d", 1.0, None, 1.0, Comparison::Equal),
            ],
            latency_ms: None,
            passed_hard_gates: false,
        };
        r.compute_gates();
        assert!(r.criteria[0].provisional && !r.criteria[0].passed);
        assert!(r.passed_hard_gates);
    }
}
