//! `.pta` asset reader and validation (contracts/asset-format.md, research R10, FR-038, FR-039).
//!
//! The reader works on bytes only. It never touches the filesystem.

use crate::casing::LexCasing;
use crate::types::AssetError;
use sha2::{Digest, Sha256};

/// Layout constants shared with the writer in `predi-tools`.
pub mod layout {
    pub const MAGIC: &[u8; 8] = b"PREDITYP";
    pub const FORMAT_VERSION: u16 = 1;
    pub const HEADER_LEN: usize = 128;
    pub const ASSET_VERSION_LEN: usize = 16;
    pub const SECTION_COUNT: usize = 6;

    pub const OFF_FORMAT: usize = 8;
    pub const OFF_RESERVED: usize = 10;
    pub const OFF_ASSET_VERSION: usize = 12;
    pub const OFF_TAU_COMPLETION: usize = 28;
    pub const OFF_TAU_NEXT: usize = 32;
    pub const OFF_ALPHA: usize = 36;
    pub const OFF_MANIFEST_SHA: usize = 40;
    pub const OFF_PAYLOAD_SHA: usize = 72;
    pub const OFF_SECTIONS: usize = 104;

    pub const SEC_LEXICON: usize = 0;
    pub const SEC_SPELLINGS: usize = 1;
    pub const SEC_FLAGS: usize = 2;
    pub const SEC_NGRAMS: usize = 3;
    pub const SEC_TOPNEXT: usize = 4;
    pub const SEC_RESERVED: usize = 5;

    /// Word ids are packed into n-gram keys with this many bits each.
    pub const ID_BITS: u32 = 21;
    pub const ID_MASK: u64 = (1 << ID_BITS) - 1;
    /// Maximum vocabulary size including the two sentence markers.
    pub const MAX_IDS: u32 = 1 << ID_BITS;

    /// Quantized scores: q = round(-log2(p) * STEPS_PER_OCTAVE), stored as u16.
    pub const STEPS_PER_OCTAVE: u32 = 1024;
    /// Maximum number of next-word ids per context list and per prefix list.
    pub const MAX_LIST: usize = 32;

    /// Word-flags byte.
    pub const FLAG_CASING_MASK: u8 = 0b0000_0011;
    pub const FLAG_REGISTER_OTHER: u8 = 0b0000_0100;
    pub const FLAG_RESERVED_MASK: u8 = 0b1111_1000;

    pub fn pack2(a: u32, b: u32) -> u64 {
        ((a as u64) << ID_BITS) | b as u64
    }

    pub fn pack3(a: u32, b: u32, c: u32) -> u64 {
        ((a as u64) << (2 * ID_BITS)) | ((b as u64) << ID_BITS) | c as u64
    }

    /// FST value for a folded key: the first word id and the number of ids sharing the key.
    pub fn lex_value(first_id: u32, count: u32) -> u64 {
        ((first_id as u64) << 8) | count as u64
    }

    pub fn pad8(n: usize) -> usize {
        (8 - n % 8) % 8
    }
}

use layout::*;

/// A validated, loaded asset.
pub struct Asset {
    data: Vec<u8>,
    pub asset_version: String,
    pub tau_completion: f32,
    pub tau_next: f32,
    pub alpha: f32,
    lexicon: fst::Map<Vec<u8>>,
    prefixes: fst::Map<Vec<u8>>,
    n_words: u32,
    spell_offsets: usize,
    spell_bytes: usize,
    flags: usize,
    // N-gram tables.
    dequant: [f64; STEPS_PER_OCTAVE as usize],
    unigram: usize,
    n_bigram: usize,
    bigram_keys: usize,
    bigram_scores: usize,
    n_trigram: usize,
    trigram_keys: usize,
    trigram_scores: usize,
    // Top-next lists.
    pool: usize,
    pool_len: usize,
    global_len: usize,
    uni_offsets: usize,
    n_bictx: usize,
    bictx_keys: usize,
    bictx_offsets: usize,
}

/// A bounds-checked little-endian cursor over the asset bytes.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        Self {
            data,
            pos: start,
            end,
        }
    }

    fn take(&mut self, n: usize) -> Result<usize, AssetError> {
        let at = self.pos;
        let next = at.checked_add(n).ok_or(AssetError::Truncated)?;
        if next > self.end {
            return Err(AssetError::Truncated);
        }
        self.pos = next;
        Ok(at)
    }

    fn u32(&mut self) -> Result<u32, AssetError> {
        let at = self.take(4)?;
        Ok(read_u32(self.data, at))
    }

    fn array(&mut self, count: usize, width: usize) -> Result<usize, AssetError> {
        let n = count.checked_mul(width).ok_or(AssetError::Truncated)?;
        self.take(n)
    }

    fn align8(&mut self) -> Result<(), AssetError> {
        let rel = self.pos;
        let pad = pad8(rel);
        let at = self.take(pad)?;
        if self.data[at..at + pad].iter().any(|&b| b != 0) {
            return Err(AssetError::Malformed("non-zero padding"));
        }
        Ok(())
    }
}

fn read_u16(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn read_u32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

fn read_u64(d: &[u8], at: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[at..at + 8]);
    u64::from_le_bytes(b)
}

fn read_f32(d: &[u8], at: usize) -> f32 {
    f32::from_bits(read_u32(d, at))
}

/// Exact power of two 2^-n for n in 0..=1022, built from bits (no platform math, G4).
fn pow2_neg(n: u32) -> f64 {
    if n > 1022 {
        0.0
    } else {
        f64::from_bits(((1023 - n) as u64) << 52)
    }
}

impl Asset {
    /// Validate and load an asset (FR-038, FR-039). No `Asset` exists after a failure.
    pub fn from_bytes(bytes: &[u8]) -> Result<Asset, AssetError> {
        if bytes.len() < MAGIC.len() {
            return Err(if bytes.is_empty() || MAGIC.starts_with(bytes) {
                AssetError::Truncated
            } else {
                AssetError::BadMagic
            });
        }
        if &bytes[..8] != MAGIC {
            return Err(AssetError::BadMagic);
        }
        if bytes.len() < HEADER_LEN {
            return Err(AssetError::Truncated);
        }
        let format = read_u16(bytes, OFF_FORMAT);
        if format != FORMAT_VERSION {
            return Err(AssetError::UnsupportedFormat {
                found: format,
                supported: FORMAT_VERSION,
            });
        }
        if read_u16(bytes, OFF_RESERVED) != 0 {
            return Err(AssetError::Malformed("reserved header bytes are not zero"));
        }

        // Section table first, so truncation is reported before a checksum mismatch.
        let mut sections = [0usize; SECTION_COUNT];
        for (i, s) in sections.iter_mut().enumerate() {
            *s = read_u32(bytes, OFF_SECTIONS + 4 * i) as usize;
        }
        if sections[SEC_RESERVED] != 0 {
            return Err(AssetError::Malformed("reserved section offset is not zero"));
        }
        let mut prev = HEADER_LEN;
        for &s in &sections[..SEC_RESERVED] {
            if s > bytes.len() {
                return Err(AssetError::Truncated);
            }
            if s < prev || s % 8 != 0 {
                return Err(AssetError::Malformed(
                    "section table out of order or misaligned",
                ));
            }
            prev = s;
        }

        let payload_sha = Sha256::digest(&bytes[HEADER_LEN..]);
        if payload_sha.as_slice() != &bytes[OFF_PAYLOAD_SHA..OFF_PAYLOAD_SHA + 32] {
            return Err(AssetError::BadChecksum);
        }

        let version_raw = &bytes[OFF_ASSET_VERSION..OFF_ASSET_VERSION + ASSET_VERSION_LEN];
        let vlen = version_raw
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(ASSET_VERSION_LEN);
        if version_raw[vlen..].iter().any(|&b| b != 0) || vlen == 0 {
            return Err(AssetError::Malformed("asset version is not NUL-padded"));
        }
        let asset_version = std::str::from_utf8(&version_raw[..vlen])
            .map_err(|_| AssetError::Malformed("asset version is not UTF-8"))?
            .to_string();

        let tau_completion = read_f32(bytes, OFF_TAU_COMPLETION);
        let tau_next = read_f32(bytes, OFF_TAU_NEXT);
        let alpha = read_f32(bytes, OFF_ALPHA);
        let unit = |x: f32| x.is_finite() && (0.0..=1.0).contains(&x);
        if !unit(tau_completion) || !unit(tau_next) {
            return Err(AssetError::Malformed("tau outside [0, 1]"));
        }
        if !(alpha.is_finite() && alpha > 0.0 && alpha <= 1.0) {
            return Err(AssetError::Malformed("alpha outside (0, 1]"));
        }

        let data = bytes.to_vec();
        let end_of = |i: usize| -> usize {
            if i + 1 < SEC_RESERVED {
                sections[i + 1]
            } else {
                data.len()
            }
        };

        // Lexicon FST: u64 length, then the FST bytes.
        let mut c = Cursor::new(&data, sections[SEC_LEXICON], end_of(SEC_LEXICON));
        let at = c.take(8)?;
        let fst_len = usize::try_from(read_u64(&data, at)).map_err(|_| AssetError::Truncated)?;
        let at = c.take(fst_len)?;
        let lexicon = load_fst(&data[at..at + fst_len], "lexicon FST")?;

        // Spellings.
        let mut c = Cursor::new(&data, sections[SEC_SPELLINGS], end_of(SEC_SPELLINGS));
        let n_words = c.u32()?;
        if n_words == 0 || n_words + 2 > MAX_IDS {
            return Err(AssetError::Malformed("vocabulary size"));
        }
        let spell_offsets = c.array(n_words as usize + 1, 4)?;
        let spell_bytes = c.pos;
        let spell_len = read_u32(&data, spell_offsets + 4 * n_words as usize) as usize;
        c.take(spell_len)?;
        let text = std::str::from_utf8(&data[spell_bytes..spell_bytes + spell_len])
            .map_err(|_| AssetError::Malformed("spellings are not UTF-8"))?;
        let mut last = 0usize;
        for i in 0..=n_words as usize {
            let o = read_u32(&data, spell_offsets + 4 * i) as usize;
            if o < last || o > spell_len || !text.is_char_boundary(o) || (i == 0 && o != 0) {
                return Err(AssetError::Malformed("spelling offsets"));
            }
            if i > 0 && o == last {
                return Err(AssetError::Malformed("empty spelling"));
            }
            last = o;
        }

        // Word flags.
        let mut c = Cursor::new(&data, sections[SEC_FLAGS], end_of(SEC_FLAGS));
        let flags = c.array(n_words as usize, 1)?;
        for &f in &data[flags..flags + n_words as usize] {
            if f & FLAG_RESERVED_MASK != 0 || f & FLAG_CASING_MASK == 3 {
                return Err(AssetError::Malformed("word flags"));
            }
        }

        // N-gram tables.
        let n_ids = n_words as usize + 2;
        let mut c = Cursor::new(&data, sections[SEC_NGRAMS], end_of(SEC_NGRAMS));
        let n_unigram = c.u32()? as usize;
        let n_bigram = c.u32()? as usize;
        let n_trigram = c.u32()? as usize;
        let _reserved = c.u32()?;
        if n_unigram != n_ids {
            return Err(AssetError::Malformed("unigram table size"));
        }
        let dq = c.array(STEPS_PER_OCTAVE as usize, 8)?;
        let mut dequant = [0f64; STEPS_PER_OCTAVE as usize];
        for (i, d) in dequant.iter_mut().enumerate() {
            *d = f64::from_bits(read_u64(&data, dq + 8 * i));
            if !(d.is_finite() && *d > 0.0 && *d <= 1.0) {
                return Err(AssetError::Malformed("dequantization table"));
            }
        }
        let unigram = c.array(n_unigram, 2)?;
        c.align8()?;
        let bigram_keys = c.array(n_bigram, 8)?;
        let bigram_scores = c.array(n_bigram, 2)?;
        c.align8()?;
        let trigram_keys = c.array(n_trigram, 8)?;
        let trigram_scores = c.array(n_trigram, 2)?;
        check_sorted(&data, bigram_keys, n_bigram, 2, n_ids)?;
        check_sorted(&data, trigram_keys, n_trigram, 3, n_ids)?;

        // Top-next lists and prefix lists.
        let mut c = Cursor::new(&data, sections[SEC_TOPNEXT], end_of(SEC_TOPNEXT));
        let pool_len = c.u32()? as usize;
        let global_len = c.u32()? as usize;
        let n_bictx = c.u32()? as usize;
        let prefix_fst_len = c.u32()? as usize;
        if global_len > pool_len.min(MAX_LIST) {
            return Err(AssetError::Malformed("global list"));
        }
        let pool = c.array(pool_len, 4)?;
        for i in 0..pool_len {
            if read_u32(&data, pool + 4 * i) as usize >= n_ids {
                return Err(AssetError::Malformed("next-word id out of range"));
            }
        }
        let uni_offsets = c.array(n_ids + 1, 4)?;
        check_offsets(&data, uni_offsets, n_ids + 1, pool_len)?;
        c.align8()?;
        let bictx_keys = c.array(n_bictx, 8)?;
        let bictx_offsets = c.array(n_bictx + 1, 4)?;
        check_sorted(&data, bictx_keys, n_bictx, 2, n_ids)?;
        check_offsets(&data, bictx_offsets, n_bictx + 1, pool_len)?;
        c.align8()?;
        let pf = c.take(prefix_fst_len)?;
        let prefixes = load_fst(&data[pf..pf + prefix_fst_len], "prefix FST")?;

        Ok(Asset {
            asset_version,
            tau_completion,
            tau_next,
            alpha,
            lexicon,
            prefixes,
            n_words,
            spell_offsets,
            spell_bytes,
            flags,
            dequant,
            unigram,
            n_bigram,
            bigram_keys,
            bigram_scores,
            n_trigram,
            trigram_keys,
            trigram_scores,
            pool,
            pool_len,
            global_len,
            uni_offsets,
            n_bictx,
            bictx_keys,
            bictx_offsets,
            data,
        })
    }

    pub fn n_words(&self) -> u32 {
        self.n_words
    }

    /// `<s>` marker id.
    pub fn bos(&self) -> u32 {
        self.n_words
    }

    /// `</s>` marker id.
    pub fn eos(&self) -> u32 {
        self.n_words + 1
    }

    pub(crate) fn lexicon(&self) -> &fst::Map<Vec<u8>> {
        &self.lexicon
    }

    /// Word ids sharing one folded key (ids are contiguous per key).
    pub fn ids_for_value(&self, value: u64) -> std::ops::Range<u32> {
        let first = (value >> 8) as u32;
        let count = (value & 0xff) as u32;
        let end = first.saturating_add(count).min(self.n_words);
        first.min(end)..end
    }

    /// Word ids whose folded form equals `folded`.
    pub fn lookup_folded(&self, folded: &str) -> std::ops::Range<u32> {
        match self.lexicon.get(folded) {
            Some(v) => self.ids_for_value(v),
            None => 0..0,
        }
    }

    pub fn spelling(&self, id: u32) -> &str {
        if id >= self.n_words {
            return "";
        }
        let a = read_u32(&self.data, self.spell_offsets + 4 * id as usize) as usize;
        let b = read_u32(&self.data, self.spell_offsets + 4 * (id as usize + 1)) as usize;
        // Validated at load: offsets are char boundaries of valid UTF-8.
        std::str::from_utf8(&self.data[self.spell_bytes + a..self.spell_bytes + b]).unwrap_or("")
    }

    pub fn flags(&self, id: u32) -> u8 {
        if id >= self.n_words {
            0
        } else {
            self.data[self.flags + id as usize]
        }
    }

    pub fn casing(&self, id: u32) -> LexCasing {
        match self.flags(id) & FLAG_CASING_MASK {
            1 => LexCasing::Capitalized,
            2 => LexCasing::Acronym,
            _ => LexCasing::Lower,
        }
    }

    pub fn register_other(&self, id: u32) -> bool {
        self.flags(id) & FLAG_REGISTER_OTHER != 0
    }

    /// Dequantize a stored score: p = frac[q mod 1024] · 2^-(q div 1024). Basic IEEE only (G4).
    pub fn dequantize(&self, q: u16) -> f64 {
        let q = q as u32;
        self.dequant[(q % STEPS_PER_OCTAVE) as usize] * pow2_neg(q / STEPS_PER_OCTAVE)
    }

    /// Unigram probability of an id (words, `<s>`, `</s>`).
    pub fn unigram(&self, id: u32) -> f64 {
        if id as usize >= self.n_words as usize + 2 {
            return 0.0;
        }
        self.dequantize(read_u16(&self.data, self.unigram + 2 * id as usize))
    }

    fn search(&self, keys: usize, n: usize, key: u64) -> Option<usize> {
        let (mut lo, mut hi) = (0usize, n);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let k = read_u64(&self.data, keys + 8 * mid);
            if k < key {
                lo = mid + 1;
            } else if k > key {
                hi = mid;
            } else {
                return Some(mid);
            }
        }
        None
    }

    /// P(w | a) if the bigram is stored.
    pub fn bigram(&self, a: u32, w: u32) -> Option<f64> {
        let i = self.search(self.bigram_keys, self.n_bigram, pack2(a, w))?;
        Some(self.dequantize(read_u16(&self.data, self.bigram_scores + 2 * i)))
    }

    /// P(w | a b) if the trigram is stored.
    pub fn trigram(&self, a: u32, b: u32, w: u32) -> Option<f64> {
        let i = self.search(self.trigram_keys, self.n_trigram, pack3(a, b, w))?;
        Some(self.dequantize(read_u16(&self.data, self.trigram_scores + 2 * i)))
    }

    fn pool_slice(&self, from: usize, to: usize) -> impl Iterator<Item = u32> + '_ {
        let to = to.min(self.pool_len);
        let from = from.min(to);
        (from..to).map(move |i| read_u32(&self.data, self.pool + 4 * i))
    }

    /// The most frequent words overall (no context).
    pub fn global_next(&self) -> impl Iterator<Item = u32> + '_ {
        self.pool_slice(0, self.global_len)
    }

    /// Ranked next words after one context id.
    pub fn next_after1(&self, a: u32) -> impl Iterator<Item = u32> + '_ {
        let (from, to) = if (a as usize) < self.n_words as usize + 2 {
            let o = self.uni_offsets + 4 * a as usize;
            (
                read_u32(&self.data, o) as usize,
                read_u32(&self.data, o + 4) as usize,
            )
        } else {
            (0, 0)
        };
        self.pool_slice(from, to)
    }

    /// Ranked next words after a two-id context.
    pub fn next_after2(&self, a: u32, b: u32) -> impl Iterator<Item = u32> + '_ {
        let (from, to) = match self.search(self.bictx_keys, self.n_bictx, pack2(a, b)) {
            Some(i) => {
                let o = self.bictx_offsets + 4 * i;
                (
                    read_u32(&self.data, o) as usize,
                    read_u32(&self.data, o + 4) as usize,
                )
            }
            None => (0, 0),
        };
        self.pool_slice(from, to)
    }

    /// Ranked frequent completions of a short folded prefix (1–3 characters), if stored.
    pub fn prefix_list(&self, folded_prefix: &str) -> impl Iterator<Item = u32> + '_ {
        let (from, to) = match self.prefixes.get(folded_prefix) {
            Some(v) => {
                let off = (v >> 8) as usize;
                (off, off + (v & 0xff) as usize)
            }
            None => (0, 0),
        };
        self.pool_slice(from, to)
    }
}

/// Load an FST and verify its internal checksum.
fn load_fst(bytes: &[u8], what: &'static str) -> Result<fst::Map<Vec<u8>>, AssetError> {
    let map = fst::Map::new(bytes.to_vec()).map_err(|_| AssetError::Malformed(what))?;
    map.as_fst()
        .verify()
        .map_err(|_| AssetError::Malformed(what))?;
    Ok(map)
}

fn check_sorted(
    data: &[u8],
    keys: usize,
    n: usize,
    arity: u32,
    n_ids: usize,
) -> Result<(), AssetError> {
    let mut prev: Option<u64> = None;
    for i in 0..n {
        let k = read_u64(data, keys + 8 * i);
        if prev.is_some_and(|p| p >= k) {
            return Err(AssetError::Malformed("n-gram keys not strictly sorted"));
        }
        if k >> (arity * ID_BITS) != 0 {
            return Err(AssetError::Malformed("n-gram key out of range"));
        }
        for j in 0..arity {
            if ((k >> (j * ID_BITS)) & ID_MASK) as usize >= n_ids {
                return Err(AssetError::Malformed("n-gram id out of range"));
            }
        }
        prev = Some(k);
    }
    Ok(())
}

fn check_offsets(data: &[u8], at: usize, n: usize, limit: usize) -> Result<(), AssetError> {
    let mut prev = 0u32;
    for i in 0..n {
        let o = read_u32(data, at + 4 * i);
        if o < prev || o as usize > limit || o - prev > MAX_LIST as u32 && i > 0 {
            return Err(AssetError::Malformed("list offsets"));
        }
        prev = o;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pow2_is_exact() {
        assert_eq!(pow2_neg(0), 1.0);
        assert_eq!(pow2_neg(1), 0.5);
        assert_eq!(pow2_neg(10), 1.0 / 1024.0);
        assert_eq!(pow2_neg(5000), 0.0);
    }

    #[test]
    fn packing() {
        assert_eq!(pack2(1, 2), (1 << 21) | 2);
        assert_eq!(pack3(1, 2, 3) & ID_MASK, 3);
        assert_eq!((pack3(1, 2, 3) >> 42) & ID_MASK, 1);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(Asset::from_bytes(&[]).err(), Some(AssetError::Truncated));
        assert_eq!(
            Asset::from_bytes(b"PRED").err(),
            Some(AssetError::Truncated)
        );
        assert_eq!(
            Asset::from_bytes(b"NOTANASSET").err(),
            Some(AssetError::BadMagic)
        );
        let mut h = vec![0u8; 200];
        h[..8].copy_from_slice(MAGIC);
        h[8] = 2;
        assert_eq!(
            Asset::from_bytes(&h).err(),
            Some(AssetError::UnsupportedFormat {
                found: 2,
                supported: 1
            })
        );
    }
}
