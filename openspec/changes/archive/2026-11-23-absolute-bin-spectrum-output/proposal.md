# Proposal

## Why

`analyze` smooths and copies the crate's in-range bins into `output` by position, so `output[i]` is the i-th in-range bin, not FFT bin i. With `min_freq=20` at 48 kHz / FFT 8192 the first four bins (0-3) are dropped, so a 440 Hz tone (true bin 75) appears at index 71. Changing `min_freq` between calls moves the smoothed peak (75 to 71), and bins outside a narrowed range keep stale values that never decay. JS visualizers cannot map index to frequency without recomputing an offset that depends on `min_freq`.

Driving requirement: the ticket (not stored in the repo) asking that `output[i]` be FFT bin i (frequency `i * sampleRate / fftSize`), with masked bins written as 0 (not smoothed), and with smoothing state held per absolute bin.

## What Changes

- `output[i]` always corresponds to absolute FFT bin i, for all i in `0..fft_size/2`, regardless of `min_freq`/`max_freq`.
- Bins outside `[min_freq, max_freq]` are written as 0 on every call (no stale tail, no decay, not smoothed).
- Smoothing (0.9 old / 0.1 new) is keyed by absolute bin, so changing `min_freq` keeps the smoothed value of bins that remain in range (peak stays at 75).
- Range bounds keep today's rule: inclusive on both ends, compared on `i * (sr / N)` in f32.
- **BREAKING (behavioural contract)**: the meaning of every `output` index changes whenever `min_freq > 0`.
- **BREAKING (export name)**: the misspelled `ouput()` export is renamed to `output()` (owner-approved). Its signature (`() -> *const f32`), pointer stability and the `fft_size/2` length are unchanged. No `ouput()` alias is kept.
- Versioning: 0.8.0 is already published to npm. `Cargo.toml` becomes the single source of truth (wasm-pack derives `pkg/package.json` from it) and is set to `0.9.0` as part of implementation. See design Decision 9.
- Config: update `openspec/config.yaml` (owner-approved) to use the `output` spelling and fix the `rules.spec` key to `rules.specs`. See design Decision 11.
- Add a native regression benchmark, `examples/bench_analyze.rs`, so `analyze` cost can be compared before and after. See design Decision 10.
- Add native `cargo test` coverage for `WasmSpectrumAnalyzer` (none exists today).

## Capabilities

### New Capabilities
- `spectrum-output`: how `analyze` populates the `ouput()` buffer: bin indexing, range masking, per-bin smoothing, and retained input validation. First spec in the project (`openspec/specs/` is empty).

### Modified Capabilities
<!-- None: no existing specs. -->

## Impact

- Code: `src/lib.rs` (`analyze`, and possibly state held in `WasmSpectrumAnalyzer`); new native tests inline or under `tests/`. `pkg/` and `test.js` are generated and not edited.
- Exported API: one name changes, `ouput` to `output` (in `src/lib.rs` and the generated `.d.ts`/glue after `wasm-pack build`). Other names and signatures are unchanged, and the pointer stays stable (`output` stays a single `Vec<f32>` of `fft_size/2` allocated in `new()`). The semantics of the values also change. Both are owner-approved in this session's request; the AGENTS.md rule against renaming without approval is satisfied by that approval, not removed.
- Consumers: external users of `@devinmdavies/wasm-fft-analyzer` that call `ouput()` fail at call time (`ouput is not a function`), and any that assume `output[0]` is the first in-range bin silently get different data. They are not discoverable from the repo.
- Docs touched by the rename: `ARCHITECTURE.md` (lines 46 and 94, known debt entry) and `AGENTS.md:94`. `openspec/config.yaml` lines 43, 80 and 106 also mention `ouput` and are updated per Decision 11.
- Performance: `analyze` is performance-sensitive. Today it costs about 4 allocations and about 120 KB per call (55 us, native, 8192 / 48 kHz); the change must not add allocations. The change is not meant to improve performance, so it is a regression guard, not a performance trade-off under AGENTS.md. Measurement method and limits are inferred in design Decision 10 for the owner to confirm.
- Dependencies: none added or upgraded.
- Versioning: tracked `Cargo.toml` says 0.7.0; the untracked local `pkg/package.json` says 0.8.0. The change sets `Cargo.toml` to 0.9.0 during implementation. No publish.

## Out of Scope

- Keeping a deprecated `ouput()` alias, or any other new export.
- Including the Nyquist bin (index N/2) in `output`; length stays `fft_size/2`.
- Changing `normalize`, dB scaling, the Hann window, or smoothing coefficients.
- Replacing the `.unwrap()` panics for invalid ranges, NaN input or bad `fft_size` with error returns, and adding constructor validation. The `audio_samples.len() < fft_size` check and truncation of longer input are preserved as is.
- Resolving the `!initialized` branch quirk beyond what per-bin smoothing requires (to be settled in design).
- Publishing to npm (`wasm-pack publish`), adding release tooling or tags, and CI or Dependabot changes.
- Criterion or any other new dependency for benchmarking, and benchmarking in WASM or a browser.
