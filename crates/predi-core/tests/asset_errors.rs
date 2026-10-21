//! Missing, corrupt and incompatible assets fail safely (T047, FR-038, FR-039, G9).

mod common;

use predi_core::asset::layout::{HEADER_LEN, OFF_FORMAT, OFF_PAYLOAD_SHA, OFF_SECTIONS, SEC_FLAGS};
use predi_core::{AssetError, Engine, PredictionRequest};
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

/// Typed text that must never appear in an error.
const SECRET: &str = "Mi cédula es secreta";

fn load(bytes: &[u8]) -> AssetError {
    // Serve a request first, so any accidental retention could leak into the error.
    let _ = common::engine().predict(&PredictionRequest::new(1, SECRET));
    match Engine::from_bytes(bytes) {
        Ok(_) => panic!("corrupt asset was accepted"),
        Err(e) => {
            let shown = format!("{e} {e:?}");
            assert!(!shown.contains(SECRET) && !shown.contains("cédula"));
            e
        }
    }
}

fn reseal(bytes: &mut [u8]) {
    let sha = Sha256::digest(&bytes[HEADER_LEN..]);
    bytes[OFF_PAYLOAD_SHA..OFF_PAYLOAD_SHA + 32].copy_from_slice(&sha);
}

#[test]
fn empty_bytes() {
    assert_eq!(load(&[]), AssetError::Truncated);
}

#[test]
fn truncated_header() {
    assert_eq!(load(&common::FIXTURE[..50]), AssetError::Truncated);
}

#[test]
fn wrong_magic() {
    let mut b = common::FIXTURE.to_vec();
    b[0] = b'X';
    assert_eq!(load(&b), AssetError::BadMagic);
}

#[test]
fn unsupported_format_version() {
    let mut b = common::FIXTURE.to_vec();
    b[OFF_FORMAT..OFF_FORMAT + 2].copy_from_slice(&2u16.to_le_bytes());
    assert_eq!(
        load(&b),
        AssetError::UnsupportedFormat {
            found: 2,
            supported: 1
        }
    );
}

#[test]
fn flipped_payload_byte() {
    let mut b = common::FIXTURE.to_vec();
    let i = b.len() - 17;
    b[i] ^= 0x40;
    assert_eq!(load(&b), AssetError::BadChecksum);
}

#[test]
fn section_offset_beyond_end() {
    let mut b = common::FIXTURE.to_vec();
    let at = OFF_SECTIONS + 4 * 4;
    let beyond = b.len() as u32 + 64;
    b[at..at + 4].copy_from_slice(&beyond.to_le_bytes());
    assert_eq!(load(&b), AssetError::Truncated);
}

#[test]
fn truncated_file() {
    let b = &common::FIXTURE[..common::FIXTURE.len() / 2];
    assert_eq!(load(b), AssetError::Truncated);
}

#[test]
fn reserved_flag_bits_rejected() {
    let mut b = common::FIXTURE.to_vec();
    let at = OFF_SECTIONS + 4 * SEC_FLAGS;
    let flags = u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as usize;
    b[flags] |= 0b1000_0000;
    reseal(&mut b);
    assert!(matches!(load(&b), AssetError::Malformed(_)));
}

#[test]
fn fixture_loads() {
    let e = common::engine();
    let info = e.info();
    assert_eq!(info.asset_version, "0.0.0-fixture");
    assert_eq!(info.format_version, 1);
}
