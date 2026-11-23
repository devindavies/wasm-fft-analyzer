# Tasks

Verification commands used throughout: `cargo test --all`, `cargo fmt --all -- --check`, `cargo clippy --all -- -D warnings`. `wasm-pack test --headless --firefox` is required for wasm-bindgen-facing changes (slice 3) but Firefox and geckodriver are not available in this environment, so report it as not verified if it cannot run.

## 1. Benchmark baseline on the untouched tree

- [x] 1.1 [BUILD] Add `examples/bench_analyze.rs` per design Decision 10 (public API plus std only, counting `#[global_allocator]`, 48 kHz, FFT 8192, range `(20, 20000)`, 440 Hz sine, 200 warm-up calls, 2000 timed calls, 5 repeats, median reported as allocations/call, bytes/call, microseconds/call). No new dependency.
  Test: `cargo run --release --example bench_analyze` prints the three figures; `cargo test --all`, `cargo fmt --all -- --check` and `cargo clippy --all -- -D warnings` pass with the example present.
- [x] 1.2 [BUILD] Run the benchmark on the unmodified `analyze` and record the baseline (allocations, bytes, median microseconds) for the PR description. Depends on 1.1; must happen before slice 2 changes `analyze`.
  Test: the recorded allocation count and bytes are stable across two runs (research reference: 4 allocations, about 120112 bytes).

## 2. Absolute-bin output with zero masking and per-bin smoothing (core slice)

Depends on 1.2 (baseline must be taken first).

- [x] 2.1 [DSP] [TEST] Add the first native `#[cfg(test)]` tests in `src/lib.rs` (design Decision 7) that read the private `output` field: 440 Hz at 48 kHz / FFT 8192 / `(20, 20000)` peaks at index 75; changing `min_freq` from 0 to 20 between calls keeps the peak at 75. Use at least 20 calls before asserting values and assert only the peak index. Write a sine-wave helper for reuse.
  Test: `cargo test --all` shows the peak-at-75 test failing on the current code (index 71) and the min-change test failing (peak moves 75 to 71), confirming the tests catch the bug.
- [x] 2.2 [DSP] Rewrite the smoothing in `analyze` (design Decisions 1 to 5): keep the length check, truncation and `.unwrap()` ahead of any state change; recover each bin as `round(freq / (sample_rate as f32 / fft_size as f32))`; skip bins `>= fft_size/2`; smooth in place into `visualize_spectrum[bin].1`; zero the regions before and after the returned range in `visualize_spectrum` and `output` with slice fills; copy smoothed bins to `output`. No new heap allocation, no new `unwrap()`/`panic!`, keep the `(f32, f32)` type and `initialized` flag.
  Test: the two tests from 2.1 now pass under `cargo test --all`; `cargo fmt --all -- --check` and `cargo clippy --all -- -D warnings` pass.
- [x] 2.3 [TEST] Add native tests mapped to the spec scenarios: `min_freq=0` peak at 75; `output[0..=3]` are 0 for `min_freq=20`; `(0, 24000)` repeated then `(100, 200)` leaves every entry outside bins 18 to 34 at 0; `min_freq=750` includes bin 128; a bin that leaves and re-enters starts from 0.1 x new magnitude, equal to a fresh instance; first-call output is 0.1 x raw; `output()` pointer equals the field's `as_ptr()` and is identical across calls with different ranges; buffer length is `fft_size/2`.
  Test: `cargo test --all` passes with all new tests; add a 44.1 kHz / 1024 case and a 96 kHz case to cover the bin recovery across sample rates.
- [x] 2.4 [TEST] Add boundary-validation tests with `#[should_panic]`: input shorter than `fft_size` (message names both lengths); `min_freq > max_freq`; `max_freq` above Nyquist; a range holding fewer than 2 bins; plus a success test for input longer than `fft_size` and a check that a panicking call leaves `output` unchanged (design Decision 5).
  Test: `cargo test --all` passes; `cargo clippy --all -- -D warnings` passes.
- [x] 2.5 [BUILD] Rerun `cargo run --release --example bench_analyze` after 2.2 and compare with the 1.2 baseline in the same session.
  Test: allocations and bytes per call equal the baseline exactly; median time per call is within +10% of the baseline. If either limit is missed, stop and escalate to the owner (AGENTS.md performance rule) instead of proceeding.

## 3. Rename `ouput()` to `output()` (API contract slice)

Depends on slice 2 (tests from 2.3 reference the pointer accessor; update them in this slice).

- [x] 3.1 [API] Rename the exported method in `src/lib.rs` to `pub fn output(&self) -> *const f32` with no alias and no `js_name`; update any test calls to it. Do not hand-edit `pkg/` or `test.js`.
  Test: `cargo test --all` passes; `grep -rn "ouput" src tests` returns nothing.
- [x] 3.2 [API] Run `wasm-pack build` and read the generated `pkg/wasm_fft_analyzer.d.ts`.
  Test: it declares `output(): number` and no `ouput`; `analyze` and constructor signatures are unchanged.
- [x] 3.3 [TEST] Run `wasm-pack test --headless --firefox` for the wasm-bindgen-facing change.
  Test: command passes; if Firefox or geckodriver is unavailable, record it as not verified in the report.
  NOT verified: Firefox and geckodriver are not installed (`which firefox geckodriver` found neither), so `wasm-pack test --headless --firefox` was not run. Left unchecked.

## 4. Docs, config, version and final integration check

Depends on slices 2 and 3.

- [x] 4.1 [BUILD] Update references to the old spelling: `ARCHITECTURE.md` (lines 46 and 94 and the known-debt entry for the version mismatch), `AGENTS.md:94`, and `openspec/config.yaml` lines 43, 80 and 106; rename `rules.spec` to `rules.specs` (design Decision 11). Change nothing else in those files.
  Test: `grep -rn "ouput" ARCHITECTURE.md AGENTS.md openspec/config.yaml` shows only the intentional "do not reintroduce `ouput`" rule; `openspec status --change absolute-bin-spectrum-output` runs without the `Unknown artifact ID in rules` warning.
- [x] 4.2 [BUILD] Set `version = "0.9.0"` in `Cargo.toml` (design Decision 9). No tag, changelog or publish.
  Test: `cargo test --all` still builds; `wasm-pack build` writes `pkg/package.json` with version 0.9.0.
- [x] 4.3 [TEST] Final integration check: run `cargo test --all`, `cargo fmt --all -- --check`, `cargo clippy --all -- -D warnings`, `cargo run --release --example bench_analyze`, and `git status` to confirm changes stay within scope and nothing is committed.
  Test: all four commands succeed, benchmark limits from 2.5 still hold, and the diff touches only `src/lib.rs`, `examples/bench_analyze.rs`, `Cargo.toml`, `ARCHITECTURE.md`, `AGENTS.md`, `openspec/config.yaml` and this change's files.

## Parallelization Strategy

### Dependencies (Must Complete First)

- **Slice 2** must wait for Slice 1 (the baseline has to be recorded on the untouched `analyze` before it changes).
- **Slice 3** must wait for Slice 2 (the 2.3 tests call the pointer accessor, which 3.1 renames in the same file).

**Independent tasks (can run in parallel):**
- Tasks 4.1 (docs and config) and 4.2 (`Cargo.toml` version) touch different files from `src/lib.rs` and from each other, so they can run alongside Slice 3 once 2.2 has landed.
- Within Slice 2, 2.3 and 2.4 are independent of each other once 2.2 passes, but both edit the same `#[cfg(test)]` module in `src/lib.rs`, so run them one after another unless the test module is split first.

**Sequential dependencies:**
- 2.1 then 2.2 then 2.5: tests must fail first, then pass, then the benchmark compares against the baseline.
- 3.1 then 3.2 and 3.3: `wasm-pack build` and the wasm test need the renamed export.
- 4.3 last: the final check needs every other task done.

**Critical path:**
Slice 1 → Slice 2 → Slice 3 → 4.3 (each step gates the next; 4.1 and 4.2 are off the critical path).

**Recommended implementation order:**
1. Slice 1 (benchmark and baseline).
2. Slice 2 in order 2.1, 2.2, then 2.3 and 2.4, then 2.5.
3. Slice 3, with 4.1 and 4.2 alongside it if desired.
4. 4.3 final check.

## Workflow follow-up

- Record entry point, baseline, command, result and the owner's approval in the PR description (performance-sensitive path).
- Leave changes uncommitted for the developer; do not run `git commit`, `git push` or `wasm-pack publish`.
- Archive the change after review requirements are satisfied.
