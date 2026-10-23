//! `.pta` asset writer, mirroring contracts/asset-format.md and `predi_core::asset` (T019).

use super::lexicon::Lexicon;
use super::ngrams::NgramModel;
use crate::util::{Result, ToolError, sha256};
use predi_core::asset::layout::*;
use predi_core::casing::LexCasing;

/// Header fields that aren't derived from the payload.
#[derive(Debug, Clone)]
pub struct HeaderParams {
    pub asset_version: String,
    pub tau_completion: f32,
    pub tau_next: f32,
    pub alpha: f32,
    pub manifest_sha256: [u8; 32],
}

/// Byte sizes of each section, for the build report.
#[derive(Debug, Clone, Default)]
pub struct SectionSizes {
    pub total: usize,
    pub sections: [usize; SECTION_COUNT],
}

fn put_u16(b: &mut Vec<u8>, v: u16) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn put_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(b: &mut Vec<u8>, v: u64) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn align8(b: &mut Vec<u8>) {
    let pad = pad8(b.len());
    b.extend(std::iter::repeat_n(0u8, pad));
}

fn flags_byte(casing: LexCasing, register_other: bool) -> u8 {
    let c = match casing {
        LexCasing::Lower => 0,
        LexCasing::Capitalized => 1,
        LexCasing::Acronym => 2,
    };
    c | if register_other {
        FLAG_REGISTER_OTHER
    } else {
        0
    }
}

/// Serialize the asset. Sections start 8-byte aligned; the payload hash covers offset 128 to end.
pub fn write_asset(
    header: &HeaderParams,
    lex: &Lexicon,
    ngrams: &NgramModel,
) -> Result<(Vec<u8>, SectionSizes)> {
    let n_words = lex.entries.len();
    if n_words == 0 {
        return Err(ToolError::invalid("empty vocabulary"));
    }
    if n_words as u64 + 2 > MAX_IDS as u64 {
        return Err(ToolError::invalid("vocabulary exceeds the 21-bit id space"));
    }
    if header.asset_version.is_empty() || header.asset_version.len() > ASSET_VERSION_LEN {
        return Err(ToolError::invalid("asset version must be 1–16 bytes"));
    }

    let mut buf = vec![0u8; HEADER_LEN];
    let mut offsets = [0u32; SECTION_COUNT];

    // Section 0: lexicon FST, folded key → (first id, count).
    offsets[SEC_LEXICON] = buf.len() as u32;
    let mut builder = fst::MapBuilder::memory();
    let mut i = 0;
    while i < n_words {
        let key = &lex.entries[i].folded;
        let mut j = i + 1;
        while j < n_words && &lex.entries[j].folded == key {
            j += 1;
        }
        if j - i > 255 {
            return Err(ToolError::invalid(
                "more than 255 words share one folded key",
            ));
        }
        builder
            .insert(key.as_bytes(), lex_value(i as u32, (j - i) as u32))
            .map_err(|e| ToolError::invalid(format!("lexicon FST: {e}")))?;
        i = j;
    }
    let fst_bytes = builder
        .into_inner()
        .map_err(|e| ToolError::invalid(format!("lexicon FST: {e}")))?;
    put_u64(&mut buf, fst_bytes.len() as u64);
    buf.extend_from_slice(&fst_bytes);
    align8(&mut buf);

    // Section 1: spellings.
    offsets[SEC_SPELLINGS] = buf.len() as u32;
    put_u32(&mut buf, n_words as u32);
    let mut text = Vec::new();
    put_u32(&mut buf, 0);
    for e in &lex.entries {
        text.extend_from_slice(e.canonical.as_bytes());
        put_u32(&mut buf, text.len() as u32);
    }
    buf.extend_from_slice(&text);
    align8(&mut buf);

    // Section 2: word flags.
    offsets[SEC_FLAGS] = buf.len() as u32;
    for e in &lex.entries {
        buf.push(flags_byte(e.casing, e.register_other));
    }
    align8(&mut buf);

    // Section 3: n-gram tables.
    offsets[SEC_NGRAMS] = buf.len() as u32;
    put_u32(&mut buf, ngrams.unigram.len() as u32);
    put_u32(&mut buf, ngrams.bigrams.len() as u32);
    put_u32(&mut buf, ngrams.trigrams.len() as u32);
    put_u32(&mut buf, 0);
    for d in &ngrams.dequant {
        put_u64(&mut buf, d.to_bits());
    }
    for &q in &ngrams.unigram {
        put_u16(&mut buf, q);
    }
    align8(&mut buf);
    for &(k, _) in &ngrams.bigrams {
        put_u64(&mut buf, k);
    }
    for &(_, q) in &ngrams.bigrams {
        put_u16(&mut buf, q);
    }
    align8(&mut buf);
    for &(k, _) in &ngrams.trigrams {
        put_u64(&mut buf, k);
    }
    for &(_, q) in &ngrams.trigrams {
        put_u16(&mut buf, q);
    }
    align8(&mut buf);

    // Section 4: top-next lists and prefix lists.
    offsets[SEC_TOPNEXT] = buf.len() as u32;
    let mut prefix_builder = fst::MapBuilder::memory();
    for (p, &(off, len)) in &ngrams.prefix_lists {
        prefix_builder
            .insert(p.as_bytes(), ((off as u64) << 8) | len as u64)
            .map_err(|e| ToolError::invalid(format!("prefix FST: {e}")))?;
    }
    let prefix_fst = prefix_builder
        .into_inner()
        .map_err(|e| ToolError::invalid(format!("prefix FST: {e}")))?;
    put_u32(&mut buf, ngrams.pool.len() as u32);
    put_u32(&mut buf, ngrams.global_len);
    put_u32(&mut buf, ngrams.bictx_keys.len() as u32);
    put_u32(&mut buf, prefix_fst.len() as u32);
    for &id in &ngrams.pool {
        put_u32(&mut buf, id);
    }
    for &o in &ngrams.uni_offsets {
        put_u32(&mut buf, o);
    }
    align8(&mut buf);
    for &k in &ngrams.bictx_keys {
        put_u64(&mut buf, k);
    }
    for &o in &ngrams.bictx_offsets {
        put_u32(&mut buf, o);
    }
    align8(&mut buf);
    buf.extend_from_slice(&prefix_fst);
    align8(&mut buf);
    // Section 5 is reserved: offset 0.

    if buf.len() > u32::MAX as usize {
        return Err(ToolError::invalid("asset exceeds 4 GiB"));
    }

    // Header.
    buf[..8].copy_from_slice(MAGIC);
    buf[OFF_FORMAT..OFF_FORMAT + 2].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    let v = header.asset_version.as_bytes();
    buf[OFF_ASSET_VERSION..OFF_ASSET_VERSION + v.len()].copy_from_slice(v);
    set_tau(&mut buf, header.tau_completion, header.tau_next);
    buf[OFF_ALPHA..OFF_ALPHA + 4].copy_from_slice(&header.alpha.to_le_bytes());
    buf[OFF_MANIFEST_SHA..OFF_MANIFEST_SHA + 32].copy_from_slice(&header.manifest_sha256);
    for (i, o) in offsets.iter().enumerate() {
        buf[OFF_SECTIONS + 4 * i..OFF_SECTIONS + 4 * i + 4].copy_from_slice(&o.to_le_bytes());
    }
    let payload = sha256(&buf[HEADER_LEN..]);
    buf[OFF_PAYLOAD_SHA..OFF_PAYLOAD_SHA + 32].copy_from_slice(&payload);

    let mut sizes = SectionSizes {
        total: buf.len(),
        sections: [0; SECTION_COUNT],
    };
    for i in 0..SEC_RESERVED {
        let end = if i + 1 < SEC_RESERVED {
            offsets[i + 1] as usize
        } else {
            buf.len()
        };
        sizes.sections[i] = end - offsets[i] as usize;
    }
    Ok((buf, sizes))
}

/// Overwrite the τ values in a written asset's header (not covered by the payload hash).
pub fn set_tau(buf: &mut [u8], tau_completion: f32, tau_next: f32) {
    buf[OFF_TAU_COMPLETION..OFF_TAU_COMPLETION + 4].copy_from_slice(&tau_completion.to_le_bytes());
    buf[OFF_TAU_NEXT..OFF_TAU_NEXT + 4].copy_from_slice(&tau_next.to_le_bytes());
}
