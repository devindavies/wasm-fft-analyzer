# wasm-fft-analyzer

A Rust library, compiled to WebAssembly, that converts a window of audio samples into a smoothed frequency spectrum for a JS visualizer. It applies a Hann window, runs an FFT with [`spectrum-analyzer`](https://crates.io/crates/spectrum-analyzer), and smooths each frequency bin over time.

## Installation

```sh
npm install @devinmdavies/wasm-fft-analyzer
```

The package is an ES module with a `.wasm` binary, built with [wasm-pack](https://github.com/rustwasm/wasm-pack) for bundlers.

## Quick start

```js
import { WasmSpectrumAnalyzer } from "@devinmdavies/wasm-fft-analyzer";
import { memory } from "@devinmdavies/wasm-fft-analyzer/wasm_fft_analyzer_bg.wasm";

const sampleRate = 48000;
const fftSize = 8192;
const analyzer = new WasmSpectrumAnalyzer(sampleRate, fftSize);

// Call this once per audio frame with at least `fftSize` samples.
function onFrame(samples /* Float32Array */) {
  analyzer.analyze(samples, 20, 20000);

  // `output()` is a pointer into WASM memory. The buffer holds fftSize / 2 floats.
  const spectrum = new Float32Array(memory.buffer, analyzer.output(), fftSize / 2);
  // spectrum[i] is the magnitude of FFT bin i, at i * sampleRate / fftSize Hz.
}
```

This snippet is based on the generated type definitions. It has not been run in a browser. The `memory` import path depends on how your bundler loads wasm-pack output.

## API

### `new WasmSpectrumAnalyzer(sample_rate: number, fft_size: number)`

Creates an analyzer. `sample_rate` is in Hz. `fft_size` is the number of samples per analysis window. The constructor sizes every internal buffer once, so `analyze` does not allocate.

### `analyze(audio_samples: Float32Array, min_freq: number, max_freq: number): void`

Analyzes the first `fft_size` samples of `audio_samples` and updates the output buffer. `min_freq` and `max_freq` are in Hz. They select which bins are filled in.

- Each filled bin is smoothed over time: 90% of the previous value plus 10% of the new one.
- Bins outside `min_freq..max_freq` are `0` after every call.
- `analyze` panics (a trap in WASM) in any of these cases: `audio_samples` is shorter than `fft_size`, `min_freq > max_freq`, `max_freq` is above the Nyquist frequency, or the range contains fewer than two bins.
- A call that panics leaves the output buffer unchanged.
- Samples beyond `fft_size` are ignored.

### `output(): number`

Returns a pointer to the output buffer in WASM linear memory: `fft_size / 2` `f32` values. `output[i]` is always FFT bin `i`, so changing `min_freq` between calls does not shift the bins. The pointer stays the same across calls. If WASM memory grows, the old `ArrayBuffer` is detached, so create a new `Float32Array` view after any call that could grow memory.

### `free(): void`

Releases the analyzer's WASM memory. Call it when you no longer need the analyzer.

## Upgrading from 0.8.x

0.9.0 contains two breaking changes:

- `ouput()` was renamed to `output()`. There is no alias for the old name.
- `output[i]` now always maps to FFT bin `i`. In 0.8.x, the index was offset by the first bin of the requested range, so a `min_freq` above 0 shifted every value. Bins outside the range are now `0` instead of stale values.

## Development

You need a Rust toolchain and [wasm-pack](https://github.com/rustwasm/wasm-pack).

```sh
wasm-pack build                       # writes pkg/ (generated, gitignored)
cargo test --all                      # native tests
cargo fmt --all -- --check            # formatting
cargo clippy --all -- -D warnings     # lints
wasm-pack test --headless --firefox   # browser tests (not run in CI)
cargo run --release --example bench_analyze   # allocations and timing for analyze
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for how the crate is structured and [AGENTS.md](AGENTS.md) for contribution rules that apply to coding agents.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE_APACHE) or [MIT license](LICENSE_MIT) at your option.
