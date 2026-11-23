# Architecture Overview

This document serves as a critical, living reference designed to equip agents and engineers with a rapid and comprehensive understanding of the codebase's architecture. Update this document as the codebase evolves.

## 1. Project Structure

wasm-fft-analyzer/
├── src/                 # Rust library crate (compiled to WASM)
│   ├── lib.rs           # WasmSpectrumAnalyzer: Hann windowing, FFT, smoothing, output buffer
│   └── utils.rs         # Panic-hook setup (console_error_panic_hook)
├── tests/web.rs         # wasm-bindgen-test browser tests (wasm32 only; currently a placeholder)
├── pkg/                 # wasm-pack output published to npm (gitignored build artifact)
├── test.js              # Ad-hoc JS script; contains generated wasm-bindgen glue code
├── openspec/            # OpenSpec workflow config, specs, and archived changes
├── .github/             # CI workflow (test, fmt, clippy) and Dependabot (cargo)
├── .appveyor.yml, .travis.yml  # Legacy CI config inherited from wasm-pack-template
├── Cargo.toml           # Crate manifest (cdylib + rlib, release profile: LTO fat, opt-level 3)
├── README.md            # Project overview (currently the unmodified wasm-pack-template README)
└── ARCHITECTURE.md      # This document

## 2. High-Level System Diagram

[JS/Browser app] --(audio samples: Float32Array)--> [WasmSpectrumAnalyzer (WASM)]
        ^                                                   |
        |                                                   +--> [spectrum-analyzer crate: FFT]
        +--(pointer to output Vec<f32> in WASM memory)------+

## 3. Core Components

### 3.1. Frontend

**Name:** None in this repo.

**Description:** Consumed by external JS/TS apps through the npm package; the host app supplies audio samples and reads results.

**Technologies:** wasm-bindgen generated ES module bindings (`pkg/wasm_fft_analyzer.js`, `.d.ts`)

**Deployment:** N/A

### 3.2. Backend Services

#### 3.2.1. WasmSpectrumAnalyzer (library)

**Name:** `wasm-fft-analyzer` crate (npm: `@devinmdavies/wasm-fft-analyzer`)

**Description:** Constructed with a sample rate and FFT size. `analyze(samples, min_freq, max_freq)` applies a precomputed Hann window into a preallocated buffer, runs `spectrum_analyzer::samples_fft_to_spectrum` over the frequency range, and applies exponential smoothing (0.9 old / 0.1 new) into `visualize_spectrum`. The result is copied to `output` (`fft_size/2` f32 values), which JS reads through `ouput()`, a raw pointer into WASM linear memory.

**Technologies:** Rust (edition 2018), wasm-bindgen 0.2.100, spectrum-analyzer 1.6.0, console_error_panic_hook 0.1.7 (default feature)

**Deployment:** Built with wasm-pack and published to npm as an ES module with `.wasm` binary.

## 4. Data Stores

None. State is in-memory only, held inside each `WasmSpectrumAnalyzer` instance (smoothed spectrum and output buffers).

## 5. External Integrations / APIs

| Service | Purpose | Integration Method |
|---------|---------|-------------------|
| npm registry | Package distribution | `wasm-pack publish` |

## 6. Deployment & Infrastructure

**Cloud Provider:** None. Library distributed as an npm package.

**Key Services Used:** npm registry

**CI/CD Pipeline:** GitHub Actions — `.github/workflows/ci.yml` (`cargo test --all`, `cargo fmt --all -- --check`, `cargo clippy --all -- -D warnings`). Legacy `.travis.yml` and `.appveyor.yml` also exist. <!-- TODO: fill in how publishing is triggered -->

**Monitoring & Logging:** Panic messages to `console.error` via `console_error_panic_hook`.

## 7. Security Considerations

**Authentication:** N/A (client-side library)

**Authorization:** N/A

**Data Encryption:** N/A

**Key Security Tools / Practices:** Dependabot for cargo dependencies (daily).

## 8. Development & Testing Environment

**Local Setup:** `wasm-pack build` to produce `pkg/`; `cargo test` for native tests.

**Testing Frameworks:** `cargo test`; `wasm-bindgen-test` (`wasm-pack test --headless --firefox`) for browser tests, currently one placeholder test.

**Code Quality Tools:** rustfmt, Clippy (`-D warnings`) in CI.

## 9. Future Considerations / Roadmap

- <!-- TODO: fill in planned architectural changes -->
- Known technical debt:
  - Public method is misspelled `ouput()` and returns a raw pointer
  - `analyze` panics on short input and `unwrap`s the FFT result
  - `normalize` has dead commented-out dB scaling
  - Crate version 0.7.0 differs from the npm package 0.8.0
  - README, `.travis.yml` and `.appveyor.yml` are unmodified template leftovers

## 10. Project Identification

**Project Name:** wasm-fft-analyzer

**Repository URL:** https://github.com/devindavies/wasm-fft-analyzer

**Primary Contact / Team:** Devin Davies

**Date of Last Update:** 2026-11-23

## 11. Glossary / Acronyms

| Term | Definition |
|------|-----------|
| WASM | WebAssembly, the compile target for the Rust crate |
| FFT | Fast Fourier Transform, used to compute the frequency spectrum |
| Hann window | Tapering function applied to samples before the FFT to reduce spectral leakage |
