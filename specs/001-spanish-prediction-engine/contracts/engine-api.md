# Contract: Engine API (`predi-core`), version 1

This is the public interface that both future clients (Windows widget and Chrome extension through
WebAssembly) and the evaluation tools use. Any breaking change bumps the major version.

## Surface

```rust
pub struct Engine { /* opaque */ }

impl Engine {
    /// Build an engine from asset bytes. No filesystem or network access.
    pub fn from_bytes(asset: &[u8]) -> Result<Engine, AssetError>;

    /// Pure with respect to text: nothing from `req` is retained after return.
    pub fn predict(&self, req: &PredictionRequest) -> PredictionResult;

    pub fn pause(&self);          // subsequent predict → status Paused, empty
    pub fn resume(&self);
    pub fn field_changed(&self);  // must be called by clients; holds no text in v1

    pub fn info(&self) -> EngineInfo; // engine version, asset version, tau values
}

// Engine: Send + Sync. Safe to call from a worker thread concurrently.
```

`PredictionRequest`, `PredictionResult`, and `Suggestion` have the fields defined in
[data-model.md](../data-model.md).

## Guarantees

| # | Guarantee | Spec |
|---|-----------|------|
| G1 | `predict` never panics for any `&str` input. Unsupported input gives an empty result with status `NotApplicable`. | FR-029 |
| G2 | At most 3 suggestions, contiguous ranks, no case- or accent-folded duplicates | FR-002, FR-013 |
| G3 | Applying any suggestion (data-model application rule) changes only the trailing partial word or appends | FR-004 |
| G4 | The same asset, engine version, and request always give the same result, on native and WASM builds alike: scoring uses only quantized scores and basic IEEE arithmetic (no platform math functions such as `ln`/`exp` at runtime), and ties break by canonical spelling | FR-007, SC-008 |
| G5 | No I/O of any kind during `predict`, `pause`, `resume`, or `field_changed` | FR-023, FR-027 |
| G6 | The work per call is bounded: input is truncated to 1,000 scalar values, and lookups are bounded | FR-031, SC-009 |
| G7 | While paused, `predict` returns an empty result with status `Paused` | FR-025 |
| G8 | No internal state changes as a result of `predict` (no learning or caching keyed by text) | FR-026 |
| G9 | `from_bytes` rejects an asset with bad magic, an unsupported format version, or a bad checksum. It returns an `AssetError` that names the problem and contains no typed text, and no `Engine` exists after a failure. | FR-038, FR-039 |
| G10 | Suggestion `text` never begins or ends with whitespace. When the request ends inside a word, all suggestions are completions. | FR-003, FR-008 |

## Client obligations (for later client features)

- Call `predict` off the thread that receives keyboard input (FR-030). Discard results whose
  `request_id` isn't the latest one.
- Call `field_changed` whenever focus moves to a different editable field in either client.
- Call `pause` when the user pauses PrediType.
- Apply suggestions only on explicit user acceptance (FR-005), using `replace_utf16` or
  `replace_chars` according to the host's text API.

## WebAssembly note

`predi-core` must compile for `wasm32-unknown-unknown`. A `wasm-bindgen` wrapper with the same
surface is a deliverable of the Chrome-extension feature, not of this one.
