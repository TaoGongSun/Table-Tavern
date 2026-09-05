# refactor.rs split final validation

## Scope

Final validation for the pure-move split of `src-tauri/src/refactor.rs` on branch `refactor-split`.

- Base commit: `1543e3ddb5033a2673486e5d256a3b999b3ca3c2`
- Immutable source blob: `860e4adc9e4f8fbe0b70f26e94306bdaefc61fb6`
- Code head validated: `a601b45392648631f3e9b36f2e4aabf6e83035ae`
- Validation environment: GitHub Actions, Windows Server 2025, Rust stable 1.98.1, Node 22

This file records evidence only. The commit that adds this note does not change production or test code, so the code tree validated below is exactly `a601b453...`.

## Mechanical integrity

A brace-aware mechanical checker compared the split files directly with `main@1543e3d:src-tauri/src/refactor.rs`.

- Original blob check: **PASS**
- Production top-level items: **20**
- Production body drift: **0**
- Production combined SHA-256: `865f2e9d15fde4b430db077c8ba993290de05a81f77b5969d2ecdaa9c9fa341e`
- Test leaves: **33**
- Test function body drift: **0**
- Test combined SHA-256: `46e91a6fbe94157d27a898827daf4455493a0f13f0d660cb708167190d61fde1`
- Caller / unrelated file changes: **0**
- Overall integrity result: **PASS**

For production comparison, the checker normalizes only the two predeclared module-plumbing visibility changes back to their original spelling before byte comparison:

1. `normalize_interface_paths`: `fn` -> `pub(super) fn`
2. `rebuild_state_fields`: `fn` -> `pub(super) fn`

No other production item was visibility-widened.

## Production ownership and facade

Final production ownership matches the baseline exactly:

- `types.rs`: 8 items
- `apply.rs`: 5 items
- `interface.rs`: 7 items
- total: 20 items

The root facade preserves all 9 original public root items:

- `RefactorCharacter`
- `RefactorInterface`
- `RefactorMechanism`
- `RefactorOutcome`
- `RefactorSelection`
- `RefactorApplySummary`
- `RefactorApplyResult`
- `apply`
- `normalize_stored_mode`

`mod.rs` contains module declarations, facade re-exports, and the two test-only module declarations only.

## Tests and cfg ledger

The 33 original `#[test]` leaves were mechanically extracted and distributed exactly as planned:

- `tests/characters.rs`: 7
- `tests/interface.rs`: 15
- `tests/mechanism.rs`: 3
- `tests/entries.rs`: 8

Allowed test-only `pub(super)` plumbing is exactly:

- `TestRoot`
- `TestRoot::new`
- `TestRoot::path`
- `seed_entry`
- `character`
- `no_player_selection`
- `apply_recorded`

Final cfg shape:

- `#[cfg(test)] mod test_support;`
- `#[cfg(test)] mod tests;`
- `tests/mod.rs` declares only `characters`, `entries`, `interface`, and `mechanism`.

No production cfg branch was added.

## Validation commands

The final Windows validation run completed successfully:

- `npm ci` — **PASS**
- `npm run build` — **PASS**
- `cargo test` — **PASS**
  - 527 tests discovered
  - 523 passed
  - 0 failed
  - 4 ignored
- `RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets` — **PASS**

The split initially emitted 8 unused-import warnings. They were cleaned up before merge, touching import lines only: each test file keeps just the `crate::data` items it uses, and the facade drops the zero-caller re-exports of `RefactorApplyResult` and `RefactorMechanism` (per plan §7-B), which `test_support` and `tests/mechanism` now take from `types` directly. Final tree: `cargo test` 535 passed with zero warnings.

## Final result

**PASS.** The original 2294-line `refactor.rs` has been replaced by the planned module directory, with production bodies and all 33 test function bodies preserved, public paths retained, visibility widening restricted to the documented whitelist, callers untouched, and the full validation suite green.
