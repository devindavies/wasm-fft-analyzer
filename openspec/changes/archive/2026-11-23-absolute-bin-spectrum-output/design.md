---
change: absolute-bin-spectrum-output
created_at: "2026-11-23T18:52:01Z"
started_at: "2026-11-23T18:58:28Z"
completed_at: "2026-11-23T19:17:46Z"
specs_touched: [spectrum-output]
decisions:
  - id: D1
    choice: Recover the absolute bin as round(freq / (sample_rate / fft_size)) from the crate's returned frequency; skip the Nyquist bin
    rationale: Exact in all 18 tested configs, no assumption about which bins the crate dropped, preserves the crate's range validation
    alternatives: [first_bin offset from ceil(min*N/sr), FrequencyLimit::All with own masking]
  - id: D2
    choice: Smooth in place by absolute bin in the existing visualize_spectrum value slot; keep tuple type and initialized flag
    rationale: Smallest diff; first-call result (0.1 x raw) unchanged
    alternatives: [Vec<f32> and drop initialized, per-bin seen flags]
  - id: D3
    choice: Zero masked bins in visualize_spectrum and output on every call; re-entering bins start from 0
    rationale: No stale tails or decay, deterministic history with no extra state, in-range bins keep their value across min_freq changes
    alternatives: [freeze state and zero only output, reset to first new value]
  - id: D4
    choice: Zero the contiguous regions before and after the returned range with slice fills; clamp last to N/2 - 1
    rationale: O(N/2) with no heap allocation
    alternatives: [merge-walk with a cursor into data(), mask Vec<bool>]
  - id: D5
    choice: Keep the length check, truncation and unwrap, and run them before any state changes
    rationale: JS-to-WASM boundary validation must be preserved; a panicking call leaves output unchanged
    alternatives: [return Result from analyze]
  - id: D6
    choice: Ship index mapping, zero-masking and per-bin smoothing as one change
    rationale: The three fixes are one contract and are inconsistent apart
    alternatives: [three separate changes]
  - id: D7
    choice: Native inline cfg(test) tests in src/lib.rs
    rationale: tests/web.rs is wasm-only and not run by CI; inline tests read the private field without unsafe
    alternatives: [tests/*.rs using the pointer and unsafe slice]
  - id: D8
    choice: Rename ouput() to output() with no alias
    rationale: Owner approved; one clean break alongside the semantic change, and a missing method fails loudly
    alternatives: [deprecated ouput alias, js_name = ouput]
  - id: D9
    choice: Cargo.toml is the version source of truth; set 0.9.0
    rationale: 0.8.0 is already published to npm; a pre-1.0 minor bump signals the break
    alternatives: [0.8.0, 0.7.1]
  - id: D10
    choice: Add examples/bench_analyze.rs as a native regression benchmark; identical allocations and bytes, median time within +10%
    rationale: Matches the research baseline method, adds no dependency, compiles in CI
    alternatives: [ignored test with global allocator, criterion dev-dependency, scratch binary]
  - id: D11
    choice: Update openspec/config.yaml for the output spelling and rename rules.spec to rules.specs
    rationale: Owner approved; config is injected into every generated artifact
    alternatives: [leave the warning, fix only the spelling]
---

# Design

## Current State

Single entry point: `WasmSpectrumAnalyzer::analyze` in `src/lib.rs:56-96`. Related: `new` (`src/lib.rs:34-54`) and the pointer accessor, currently misspelled `ouput()` (`src/lib.rs:98-100`). `normalize` (`src/lib.rs:18-29`) is an identity and is only called by `analyze`.

Data flow today (48 kHz, N=8192, range 20..20000):

```
audio_samples (Float32Array copied into WASM memory by wasm-bindgen)
   |  len < N -> panic ; longer input truncated to [..N]
   v
hann_buf (preallocated, windowed)
   v
samples_fft_to_spectrum(.., Range(min,max), normalize).unwrap()   <- allocates result Vec
   |  data(): only in-range bins, 3410 entries, first = bin 4 (23.4 Hz)
   v
zip by POSITION into visualize_spectrum[0..len]   (0.9*old + 0.1*new)
   |  positions len..N/2 never written (stale forever)
   v
output[i] = visualize_spectrum[i].1   (positional copy, N/2 = 4096 entries)
   v
ouput() [to become output()] -> *const f32 into WASM linear memory (no length exposed)
```

Consequences measured on native release builds: a 440 Hz tone (true bin 75) lands at index 71; changing `min_freq` from 0 to 20 moves the smoothed peak from index 75 to 71; after a narrow call, positions beyond the range keep old values with no decay. The first call stores 0.1 x raw because `!initialized` writes the frequency slot of the tuple, not the value slot, so the value slot always starts from 0.

Facts the design relies on (verified for 44.1/48/96 kHz, N 1024..32768, 18 configs):
- The crate's bin frequency is `i * (sr/N)` in f32; bounds are inclusive; `round(f / (sr/N))` recovers `i` exactly in every config tested.
- The crate returns `N/2 + 1` bins when the range includes Nyquist; `output` has `N/2` entries, so the Nyquist bin is already dropped by the zip.
- Invalid or too-narrow limits (fewer than 2 bins), NaN samples and bad `fft_size` all panic at the single `.unwrap()`.

## Desired End State

Changes: `output[i]` is bin `i`; out-of-range entries are 0 every call; smoothing state is per absolute bin. See `specs/spectrum-output/spec.md`.

Also changes: the pointer accessor is exported as `output()` instead of `ouput()` (Decision 8), and the crate version is set to 0.9.0 (Decision 9).

Stays the same: `new` and `analyze` names and signatures, `output()` returning `*const f32` to the start of one `Vec<f32>` of `fft_size/2` allocated in `new()`, the sample-length check and truncation, the 0.9/0.1 coefficients, `normalize`, panics for invalid limits, and the first-call result (0.1 x raw).

```
bin:      0 1 2 3 | 4 ............ 3413 | 3414 ......... 4095
range:    masked  | in range            | masked
output:   0 0 0 0 | smoothed values     | 0 ...........  0
index == bin always; length N/2; Nyquist (N/2) still not represented
```

Memory layout and pointer validity: `output` is one contiguous `f32` array, `fft_size/2` x 4 bytes, at an offset in WASM linear memory. The pointer is fixed from `new()` until the instance is freed (never resized; verified identical across 5 calls natively). `analyze` allocates (input array copy, FFT buffers), so linear memory may grow, which detaches any JS `Float32Array` built over the old `memory.buffer`. The pointer value is still valid, but consumers must re-create their view from `memory.buffer` after growth. This is existing behaviour and is not changed; the length is a consumer convention (`fft_size/2`) and remains so.

## Patterns to Follow

- `src/lib.rs:60-65`: iterator-based, allocation-free writes into preallocated buffers.
- `src/lib.rs:76`: copy needed scalars before the closure to avoid borrowing `self`.

## Patterns to Avoid

- No new `unwrap()` or `panic!` on paths reachable from JS (existing ones are preserved, not extended).
- No `debug_assert!` as the only guard for index bounds; absolute bin indices are bounds-checked by slice access or an explicit `< fft_size/2` test.
- Do not keep or reintroduce the `ouput` spelling in any export or doc (Decision 8).

## Resolved Decisions

### Decision 1: Recover the absolute bin from the crate's frequency value
**Choice:** For each returned `(freq, val)`, compute `bin = round(freq / (sample_rate as f32 / fft_size as f32))`; skip entries with `bin >= fft_size/2` (the Nyquist bin).
**Rationale:** Exact in every tested config, needs no assumption about which bins the crate dropped, and stays correct if the crate's inclusion rule changes slightly. Spec range semantics are therefore whatever the crate returns, which already matches `i*sr/N` inclusive.
**Alternatives Considered:** (a) `first_bin = ceil(min*N/sr)` offset plus position: re-implements the crate's f32 boundary rule and can disagree at boundaries. (b) Drop the crate's range filter, call with `FrequencyLimit::All` and mask ourselves: changes the panic behaviour for invalid or narrow limits and does extra FFT-result work; rejected to preserve validation.

### Decision 2: Smooth in place by absolute bin using the existing `visualize_spectrum` value slot
**Choice:** Index `visualize_spectrum[bin].1` directly instead of zipping by position. Keep the `(f32, f32)` type and the `initialized` flag untouched.
**Rationale:** Smallest diff; first-call output (0.1 x raw) is unchanged. The frequency slot and `initialized` branch stay a no-op as today; cleaning them is separate (out of scope).
**Alternatives Considered:** Change to `Vec<f32>` and drop `initialized` (cleaner, but touches unrelated private state; defer). Per-bin "seen" flags (unneeded, see Decision 3).

### Decision 3: Masked bins are zeroed in both `visualize_spectrum` and `output`; re-entry starts from 0
**Choice:** Each call, bins outside the returned range get value 0 in `visualize_spectrum` and `output`. A bin that re-enters therefore begins from 0, giving `0.1 x new` on first in-range call, identical to a fresh instance.
**Rationale:** Meets "written as 0 (not smoothed)", no decay or stale tails, and defines history deterministically with no extra state. Bins that stay in range keep their value across `min_freq` changes (the peak stays at 75).
**Alternatives Considered:** Freeze state and only zero `output` (re-entry would show a stale pre-exit value that is unrelated to the current signal, producing a visible jump); reset to the first new value (needs per-bin flags and a ramp discontinuity relative to the first-call behaviour).

### Decision 4: Contiguous-range zeroing without allocation
**Choice:** The data is a contiguous run of bins `first..=last`. Zero `0..first` and `last+1..N/2` of `visualize_spectrum` values, smooth `first..=last` by bin, then copy to `output`. Handle the Nyquist bin by clamping `last` to `N/2 - 1`.
**Rationale:** O(N/2) with no heap allocation; only slice fills over preallocated buffers.
**Alternatives Considered:** Merge-walk over all N/2 bins with a cursor into `data()` (one pass but branchier); a mask `Vec<bool>` (extra allocation/state).

### Decision 5: Keep validation and panics as is
**Choice:** The `len < fft_size` check, truncation, and the `.unwrap()` on the crate result stay in place and run before any state is modified.
**Rationale:** AGENTS.md forbids simplifying JS-to-WASM boundary validation. Running them first means a panicking call leaves `output` unchanged.
**Alternatives Considered:** Convert to `Result` (changes the exported signature; needs owner approval and is out of scope).

### Decision 6: One change, three fixes together
**Choice:** Ship index mapping, zero-masking and per-bin smoothing as one change.
**Rationale:** The three are one contract: absolute indexing alone still leaves stale tails and position-bound smoothing, and per-bin smoothing is only meaningful with absolute indexing. Owner can still split (see Open Questions).
**Alternatives Considered:** Three separate changes (each leaves an inconsistent intermediate contract).

### Decision 7: Native tests inline in `src/lib.rs`
**Choice:** Add `#[cfg(test)]` tests in `src/lib.rs` that read the private `output` field directly; do not use `tests/web.rs`.
**Rationale:** `tests/web.rs` is wasm-only and invisible to CI's `cargo test --all`; inline tests avoid an unsafe pointer read. A separate native test also checks `output()` equals the field's `as_ptr()`.
**Alternatives Considered:** `tests/*.rs` using `ouput()` plus `unsafe slice::from_raw_parts` (valid, but adds unsafe to the crate's tests).

### Decision 8: Rename `ouput()` to `output()` with no alias
**Choice:** Rename the method in `src/lib.rs` to `output` (`pub fn output(&self) -> *const f32`). The private field is also `output`; Rust allows a field and a method of the same name, and wasm-bindgen exports only the method (fields are private). Update `ARCHITECTURE.md` and `AGENTS.md:94` references. Do not add `#[wasm_bindgen(js_name = ouput)]` or a second export.
**Rationale:** Owner asked for the rename. The semantic change already breaks every consumer that reads `output[0]`, so one clean break with one version bump costs less than carrying a misspelled alias. Fail-fast at call time (`ouput is not a function`) is also safer than silently serving different data under the old name.
**Alternatives Considered:** Keep `ouput` as a deprecated alias for one release (softer migration, but keeps the misspelling in the contract and adds a second export, which is new API surface); `js_name = "ouput"` on a Rust `output` (keeps the JS break away, but then the Rust rename is pointless).

### Decision 9: `Cargo.toml` is the version source of truth; set 0.9.0
**Choice:** Set `version = "0.9.0"` in `Cargo.toml` as an implementation task and treat `pkg/package.json` as derived (wasm-pack regenerates it from `Cargo.toml`; `pkg/` is gitignored and never edited by hand). No tag, changelog or publish is added.
**Rationale:** Tracked history stops at 0.7.0 (`f4b1f0c`), but the owner confirmed 0.8.0 is already published to npm, so `Cargo.toml` is behind the published version. Pre-1.0 semver treats a minor bump as breaking, and 0.9.0 is the next free minor. This removes the mismatch listed in `ARCHITECTURE.md` known debt (update that line).
**Alternatives Considered:** 0.8.0 (already published, cannot be reused); 0.7.1 (behind the published version and understates a breaking change).

### Decision 10: Native benchmark example as a regression guard
**Choice:** Add `examples/bench_analyze.rs`, run with `cargo run --release --example bench_analyze`. It uses only the public API (`new`, `analyze`) and std, with a counting `#[global_allocator]` and `Instant`. Fixed config: 48 kHz, FFT 8192, `(20, 20000)`, a 440 Hz sine, 200 warm-up calls, then 2000 timed calls, repeated 5 times with the median reported. Output: allocations per call, bytes per call, microseconds per call.
**Why it is not a trade-off:** AGENTS.md defines a trade-off as accepting a cost to reduce latency, CPU, memory or allocations. This change is not meant to reduce any of them, so the benchmark guards against regression instead. The PR description still records the entry point, baseline, command and result.
**Procedure:** Run on the untouched tree first and record the baseline (research measured 4 allocations, about 120112 bytes, about 55 us), then after the change.
**Limits:** allocations and bytes per call must equal the baseline exactly (they are deterministic); median time per call must stay within +10% of the baseline measured on the same machine in the same session. The research baseline is context only, not the comparison.
**Rationale:** It matches how the baseline was already taken, adds no dependency, and an example is built by `cargo test --all` and `cargo clippy --all`, so CI keeps it compiling without running it. Native time is a proxy for WASM, which has no harness here.
**Alternatives Considered:** An `#[ignore]` test in `src/lib.rs` (a global allocator would apply to every test in the binary and need `--test-threads=1`); `criterion` as a dev-dependency (new dependency, needs approval); a scratch binary outside the repo (not reproducible, and the scratchpad can be cleared).

### Decision 11: Update `openspec/config.yaml` for the rename and the rule key
**Choice:** Line 43 reads `output()` instead of `ouput() (sic)`. Line 80 is rewritten to say not to reintroduce the misspelled `ouput` export. Line 106 reads `output()`. The `rules.spec` key is renamed to `rules.specs` so OpenSpec stops warning `Unknown artifact ID in rules: "spec"`. No other config changes.
**Rationale:** Owner approved. The config is injected into every generated artifact, so a stale spelling would reintroduce the old name. The rule contents are unchanged, only the key.
**Alternatives Considered:** Leave the warning (the spec rules never apply to generated specs); fix only the spelling (leaves the warning).

Test convergence: 440 Hz at 48 kHz/8192 peaks at index 75 from the first call (o[76]/o[75] about 0.57); use at least 20 calls (about 88% of raw) before asserting values, and assert index only for the peak.

## Performance Implications

- `analyze` is performance-sensitive (AGENTS.md). Today: about 4 allocations and about 120 KB per call, 55 us native (8192, 48 kHz, 2000 calls). All allocations come from the crate FFT result, the `frequency_vec` collect and `working_buffer`, none from this crate's own code.
- This change adds no allocation. It adds at most two slice fills over the masked region (up to N/2 f32 writes) and replaces a positional zip with indexed writes, plus one f32 divide and round per in-range bin (or one hoisted divisor with a per-bin multiply-by-reciprocal avoided to keep exactness).
- The change is not intended to improve performance, so it is a regression guard rather than a trade-off under AGENTS.md. Measurement and limits are in Decision 10: identical allocations and bytes per call, median time within +10% of a baseline taken in the same session.
- wasm-bindgen boundary: signatures are identical, so the `&[f32]` argument copy into linear memory and the returned raw pointer behave as before. No data is copied across the boundary on output.
- Do not apply dB scaling here: zero as "masked" is only unambiguous for linear magnitudes. If dB is ever enabled, masked bins would need a sentinel (recorded as a boundary, not decided here).

## Risks / Trade-offs

- [Consumers break twice: `ouput` missing, and index semantics] -> Both are approved and ship in one minor bump (0.9.0); consumers are not discoverable from the repo.
- [`wasm-pack build` output (`pkg/`, `.d.ts`) still shows `ouput` until rebuilt] -> `pkg/` is generated and gitignored; rebuild before any publish and check the `.d.ts` exposes `output()` only.
- [`round(freq/res)` mis-maps in untested configs, e.g. sizes above 32768 or unusual sample rates] -> Clamp index to `< N/2`; tests cover 44.1/48/96 kHz at several sizes.
- [Zero-fill cost on very narrow ranges] -> Bounded by N/2 writes; checked by the Decision 10 benchmark.
- [Re-entering bin ramps from 0 over about 20 calls] -> Matches first-call behaviour; accepted.
- [`wasm-pack test --headless --firefox` cannot run here (no Firefox/geckodriver)] -> Report as not verified; native tests cover the logic.

## Migration Plan

No data migration. Consumers rename `ouput()` to `output()` and drop any hard-coded offset, reading `output[bin]` directly. The version goes to 0.9.0 in `Cargo.toml`; publishing is not part of this change. Rollback is reverting the commit.

## Open Questions

No unresolved questions.

Resolved on review: the owner accepted the Decision 10 benchmark as a tracked file, and approved the design with the three fixes as one change (Decision 6).
