//! Small shared helpers.

use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

/// An error that carries the process exit code defined in contracts/cli.md.
#[derive(Debug)]
pub struct ToolError {
    pub code: i32,
    pub message: String,
}

impl ToolError {
    /// Invalid input (exit code 2).
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: message.into(),
        }
    }

    /// Test split leaked into the build input (exit code 3).
    pub fn leakage(message: impl Into<String>) -> Self {
        Self {
            code: 3,
            message: message.into(),
        }
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ToolError {}

pub type Result<T> = std::result::Result<T, ToolError>;

pub fn read_to_string(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| ToolError::invalid(format!("cannot read {}: {e}", path.display())))
}

pub fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path)
        .map_err(|e| ToolError::invalid(format!("cannot read {}: {e}", path.display())))
}

pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir)
            .map_err(|e| ToolError::invalid(format!("cannot create {}: {e}", dir.display())))?;
    }
    std::fs::write(path, bytes)
        .map_err(|e| ToolError::invalid(format!("cannot write {}: {e}", path.display())))
}

/// Non-empty, non-comment lines of a one-entry-per-line list file.
pub fn read_list(path: &Path) -> Result<Vec<String>> {
    Ok(read_to_string(path)?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(predi_core::normalize::nfc)
        .collect())
}

/// Like [`read_list`], but a missing file is an empty list.
pub fn read_list_optional(path: &Path) -> Result<Vec<String>> {
    if path.exists() {
        read_list(path)
    } else {
        Ok(Vec::new())
    }
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&sha256(bytes))
}

/// SHA-256 of a corpus message's NFC-normalized text (the split key).
pub fn message_hash(text: &str) -> String {
    sha256_hex(predi_core::normalize::nfc(text).as_bytes())
}
