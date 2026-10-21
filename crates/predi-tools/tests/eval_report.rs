//! The evaluation report follows contracts/eval-report.schema.json and contains no corpus text
//! (T051, FR-037).

use predi_tools::build::sources::Split;
use predi_tools::eval::{EvalOptions, run};
use predi_tools::util::message_hash;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MESSAGES: &[&str] = &[
    "Buenos días, {NOMBRE}. ¿En qué le puedo ayudar?",
    "Muchas gracias por su paciencia, ya reviso su caso.",
    "Su pedido {NUMERO_PEDIDO} fue enviado a Bogotá.",
    "Le pedimos una disculpa por las molestias ocasionadas.",
    "Quedamos atentos a cualquier inquietud.",
    "Su contraseña fue restablecida con éxito.",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn setup() -> (PathBuf, EvalOptions) {
    let dir = std::env::temp_dir().join(format!("predi-eval-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("corpus")).unwrap();
    let mut tsv = String::from("category\ttext\n");
    let mut splits = String::from("message_sha256\tsplit\n");
    for m in MESSAGES {
        tsv.push_str(&format!("otro\t{m}\n"));
        splits.push_str(&format!("{}\ttest\n", message_hash(m)));
    }
    std::fs::write(dir.join("corpus/messages.tsv"), tsv).unwrap();
    std::fs::write(dir.join("corpus/splits.tsv"), splits).unwrap();
    std::fs::write(
        dir.join("machine.toml"),
        "model = \"test\"\ncpu = \"test\"\ncores = 4\nbase_clock_ghz = 2.5\nram_gb = 8\n\
         os_build = \"test\"\npower_plan = \"test\"\nis_reference = false\n",
    )
    .unwrap();
    let r = root();
    let opts = EvalOptions {
        asset: r.join("crates/predi-core/fixtures/mini.pta"),
        corpus: dir.join("corpus"),
        split: Split::Test,
        cases: r.join("data/correctness/cases.jsonl"),
        machine: dir.join("machine.toml"),
        words: r.join("crates/predi-core/fixtures/mini-sources/words.txt"),
        domain_words: r.join("data/lexicon/domain-words.txt"),
        allowlist: r.join("data/allowlist-proper-nouns.txt"),
        given_names: r.join("data/given-names.txt"),
        check_determinism: true,
        only: None,
    };
    (dir, opts)
}

fn all_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| all_strings(x, out)),
        Value::Object(o) => o.iter().for_each(|(k, x)| {
            out.push(k.clone());
            all_strings(x, out)
        }),
        _ => {}
    }
}

fn is_case_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

#[test]
fn report_follows_schema_and_has_no_corpus_text() {
    let (dir, opts) = setup();
    let report = run(&opts, Instant::now()).expect("evaluation runs");
    let path = report.write(&dir.join("reports")).expect("report written");
    let file = std::fs::read_to_string(&path).unwrap();
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("eval-") && name.ends_with("Z.json"),
        "{name}"
    );
    let v: Value = serde_json::from_str(&file).unwrap();

    for key in [
        "generated_at",
        "engine_version",
        "asset_version",
        "corpus_manifest_sha256",
        "machine",
        "criteria",
        "passed_hard_gates",
    ] {
        assert!(v.get(key).is_some(), "missing {key}");
    }
    let sha = v["corpus_manifest_sha256"].as_str().unwrap();
    assert!(
        sha.len() == 64
            && sha
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
    );
    for key in [
        "model",
        "cpu",
        "cores",
        "base_clock_ghz",
        "ram_gb",
        "os_build",
        "power_plan",
    ] {
        assert!(v["machine"].get(key).is_some(), "machine.{key}");
    }

    let criteria = v["criteria"].as_array().unwrap();
    let mut ids = Vec::new();
    for c in criteria {
        let id = c["id"].as_str().unwrap();
        let n: u32 = id
            .strip_prefix("SC-0")
            .and_then(|x| x.parse().ok())
            .unwrap_or(0);
        assert!(id.len() == 6 && (1..=15).contains(&n), "{id}");
        ids.push(id.to_string());
        for key in [
            "description",
            "value",
            "threshold",
            "comparison",
            "provisional",
            "passed",
        ] {
            assert!(c.get(key).is_some(), "{id}.{key}");
        }
        assert!(c["value"].is_number() && c["threshold"].is_number());
        assert!(matches!(c["comparison"].as_str(), Some(">=" | "<=" | "==")));
        if let Some(detail) = c.get("detail").and_then(Value::as_object) {
            for (k, d) in detail {
                let ok = d.is_number()
                    || d.is_boolean()
                    || d.as_array()
                        .is_some_and(|a| a.iter().all(|x| x.as_str().is_some_and(is_case_id)));
                assert!(ok, "{id}.detail.{k} must be a number, flag or case-id list");
            }
        }
        let provisional =
            ["SC-001", "SC-002", "SC-003", "SC-004", "SC-014", "SC-015"].contains(&id);
        assert_eq!(c["provisional"].as_bool(), Some(provisional), "{id}");
    }
    // Every criterion measured in-process is reported (SC-012 is verified externally).
    for n in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15] {
        assert!(ids.contains(&format!("SC-{n:03}")), "SC-{n:03} missing");
    }

    // Provisional criteria never fail the run: the gate covers only hard criteria.
    let hard_ok = criteria
        .iter()
        .filter(|c| c["provisional"] == Value::Bool(false))
        .all(|c| c["passed"] == Value::Bool(true));
    assert_eq!(v["passed_hard_gates"].as_bool(), Some(hard_ok));

    // FR-037: no corpus text anywhere (messages, nor any of their sentences or long fragments).
    let mut strings = Vec::new();
    all_strings(&v, &mut strings);
    for m in MESSAGES {
        for s in &strings {
            assert!(!s.contains(m), "report contains a corpus message");
        }
        for fragment in m
            .split(['.', ',', '?', '¿'])
            .map(str::trim)
            .filter(|f| f.len() >= 12)
        {
            assert!(
                !file.contains(fragment),
                "report contains corpus text: {fragment}"
            );
        }
    }
}
