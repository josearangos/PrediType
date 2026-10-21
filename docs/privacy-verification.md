# Privacy Verification (SC-012)

SC-012 requires that during a full corpus replay **0 bytes of typed text are written to disk** and
**0 network connections are attempted**. The evaluation process can't observe itself reliably, so
this is checked from outside the process:

- **Windows reference computer**: Sysinternals Process Monitor (this document). Required for the
  baseline (T065, T069).
- **Linux CI**: `scripts/check-privacy.sh` runs the same replay under `strace` on every push.

Static guarantees back both checks: the engine crate has no filesystem, network, process or
logging calls (Clippy `disallowed-*` lints in `crates/predi-core/clippy.toml`), and `cargo deny`
bans networking crates from the workspace.

## Windows procedure (Process Monitor)

### Prerequisites

- The reference computer described in `docs/reference-computer.toml`.
- Process Monitor from Sysinternals (`Procmon64.exe`).
- A release build: `cargo build --release -p predi-tools`.
- The asset `assets/es-CO-1.0.0.pta` and the corpus with its frozen `splits.tsv`.

### 1. Configure the capture

1. Start `Procmon64.exe` as administrator and stop the capture (Ctrl+E).
2. Clear the display (Ctrl+X).
3. Open **Filter → Filter…** (Ctrl+L) and add these rules, then **Apply**:

   | Column | Relation | Value | Action |
   |--------|----------|-------|--------|
   | Process Name | is | `predi-eval.exe` | Include |
   | Operation | is | `WriteFile` | Include |
   | Operation | is | `CreateFile` | Include |
   | Operation | is | `SetRenameInformationFile` | Include |
   | Operation | begins with | `TCP` | Include |
   | Operation | begins with | `UDP` | Include |

4. Turn off **Options → Drop Filtered Events** so nothing is lost, then start the capture
   (Ctrl+E).

### 2. Run the replay

From the repository root, in PowerShell:

```text
target\release\predi-eval.exe --asset assets\es-CO-1.0.0.pta --corpus data\corpus\es-co-cs `
  --split test --machine docs\reference-computer.toml --out reports\ --only performance
```

Stop the capture (Ctrl+E) when the command finishes.

### 3. Check the events

| Event | Expected |
|-------|----------|
| `TCP *` / `UDP *` | **None.** Any network event fails SC-012. |
| `WriteFile` | Only on `reports\eval-<timestamp>.json`. Console output goes to the console handle, not a file. |
| `CreateFile` with *Desired Access* containing `Write` or *Disposition* `Create`/`OverwriteIf` | Only for `reports\eval-<timestamp>.json` (and the `reports\` directory). |
| `CreateFile` for reading | Allowed: the asset, `messages.tsv`, `splits.tsv`, `reference-computer.toml`, and system DLLs. |
| `SetRenameInformationFile` | **None.** |

The report itself is evaluation output that contains only metrics, versions, hardware details and
case ids (FR-037), so it doesn't count against SC-012.

To double-check, use **Tools → File Summary** and confirm the only path with bytes written is the
report, and **Tools → Network Summary** shows no entries.

### 4. Record the result

Save the capture as `privacy-<date>.PML` (keep it outside the repository: it lists local paths)
and record the outcome below, together with the report file name of the same run.

## Results log

| Date | Machine (model) | Engine / asset version | Network events | Writes outside `reports\` | Result | Recorded by |
|------|-----------------|------------------------|----------------|---------------------------|--------|-------------|
| _pending (T069)_ | | | | | | |
