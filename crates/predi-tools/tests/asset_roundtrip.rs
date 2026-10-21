//! A written asset reads back identically through `predi_core` (T019).

use predi_core::asset::Asset;
use predi_core::casing::LexCasing;
use predi_tools::build::lexicon::{LexiconInputs, Text, build_lexicon, count_words};
use predi_tools::build::ngrams::{NgramParams, build_ngrams};
use predi_tools::build::pii::Allowlist;
use predi_tools::build::writer::{HeaderParams, write_asset};
use std::collections::HashSet;

fn set(words: &[&str]) -> HashSet<String> {
    words.iter().map(|s| s.to_string()).collect()
}

#[test]
fn round_trip() {
    let texts: Vec<Text> = [
        "Buenos días, ¿en qué le puedo ayudar?",
        "Buenos días, gracias por su paciencia.",
        "Gracias por su compra en Bogotá.",
        "Pague por PSE, por favor.",
        "¿Puedo ayudarte?",
        "Sí, si quiere.",
    ]
    .iter()
    .map(|t| Text {
        text: t.to_string(),
        weight: 1.0,
    })
    .collect();
    let dict = set(&[
        "buenos",
        "días",
        "en",
        "qué",
        "le",
        "puedo",
        "ayudar",
        "gracias",
        "por",
        "su",
        "paciencia",
        "compra",
        "pague",
        "favor",
        "ayudarte",
        "sí",
        "si",
        "quiere",
    ]);
    let domain = HashSet::new();
    let allow = Allowlist::new(&["Bogotá".to_string(), "PSE".to_string()]);
    let flags = set(&["ayudarte"]);
    let (stats, _) = count_words(&texts);
    let inputs = LexiconInputs {
        dictionary: &dict,
        domain_words: &domain,
        allowlist: &allow,
        register_flags: &flags,
    };
    let (lex, _) = build_lexicon(&stats, &inputs);
    let ngrams = build_ngrams(
        &texts,
        &lex,
        NgramParams {
            min_bigram: 1.0,
            min_trigram: 1.0,
        },
    );
    let header = HeaderParams {
        asset_version: "9.8.7-test".into(),
        tau_completion: 0.25,
        tau_next: 0.125,
        alpha: 0.4,
        manifest_sha256: [7; 32],
    };
    let (bytes, sizes) = write_asset(&header, &lex, &ngrams).expect("write");
    assert_eq!(sizes.total, bytes.len());
    assert_eq!(bytes.len() % 8, 0);

    let a = Asset::from_bytes(&bytes).expect("read");
    assert_eq!(a.asset_version, "9.8.7-test");
    assert_eq!(a.tau_completion, 0.25);
    assert_eq!(a.tau_next, 0.125);
    assert_eq!(a.alpha, 0.4);
    assert_eq!(a.n_words() as usize, lex.entries.len());

    for (id, e) in lex.entries.iter().enumerate() {
        let id = id as u32;
        assert_eq!(a.spelling(id), e.canonical);
        assert_eq!(a.casing(id), e.casing);
        assert_eq!(a.register_other(id), e.register_other);
        assert!(a.lookup_folded(&e.folded).contains(&id));
        assert_eq!(
            ngrams.unigram[id as usize] == u16::MAX,
            a.unigram(id) == 0.0
        );
    }
    // si/sí share a folded key; both ids come back.
    let si = a.lookup_folded("si");
    assert_eq!(si.len(), 2);
    assert_eq!(a.spelling(lex.id_of("bogotá").unwrap()), "Bogotá");
    assert_eq!(a.casing(lex.id_of("pse").unwrap()), LexCasing::Acronym);

    for &(k, q) in &ngrams.bigrams {
        let (x, y) = ((k >> 21) as u32, (k & 0x1f_ffff) as u32);
        assert_eq!(a.bigram(x, y), Some(a.dequantize(q)));
    }
    for &(k, q) in &ngrams.trigrams {
        let (x, y, z) = (
            (k >> 42) as u32,
            ((k >> 21) & 0x1f_ffff) as u32,
            (k & 0x1f_ffff) as u32,
        );
        assert_eq!(a.trigram(x, y, z), Some(a.dequantize(q)));
    }
    assert_eq!(a.global_next().count(), ngrams.global_len as usize);
    let buenos = lex.id_of("buenos").unwrap();
    let dias = lex.id_of("días").unwrap();
    assert_eq!(a.next_after1(buenos).collect::<Vec<_>>(), vec![dias]);
    assert_eq!(
        a.next_after2(a.bos(), buenos).collect::<Vec<_>>(),
        vec![dias]
    );
    assert!(
        a.prefix_list("gr")
            .any(|id| id == lex.id_of("gracias").unwrap())
    );
    // Flagged words never appear in next-word or prefix lists.
    let flagged = lex.id_of("ayudarte").unwrap();
    assert!(!a.prefix_list("ay").any(|id| id == flagged));
}
