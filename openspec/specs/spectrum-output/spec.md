---
related: []
last_touched_by: absolute-bin-spectrum-output
last_touched_on: 2026-11-23
---

# spectrum-output Specification

## Purpose

Defines how the analyzer fills the buffer exposed through `output()` so that JS visualizers can map each index to a frequency without knowing the configured frequency range.

## Requirements

### Requirement: Output index equals FFT bin index
For every call to `analyze`, `output[i]` SHALL hold the smoothed magnitude of FFT bin `i`, whose frequency is `i * sample_rate / fft_size`, for `i` in `0..fft_size/2`. The index SHALL NOT depend on `min_freq` or `max_freq`.

#### Scenario: Tone lands on its true bin with a non-zero minimum
- **WHEN** a 440 Hz sine is analyzed at 48 kHz, FFT size 8192, `min_freq=20`, `max_freq=20000` for enough calls to converge
- **THEN** the largest value in the output is at index 75

#### Scenario: Minimum of zero
- **WHEN** the same input is analyzed with `min_freq=0`
- **THEN** the largest value is at index 75 and bin 0 is a candidate for a non-zero value

### Requirement: Out-of-range bins are zero
Every call SHALL write 0 to each `output` entry whose bin frequency is below `min_freq` or above `max_freq`. Such entries SHALL NOT be smoothed, decayed or left at a previous value.

#### Scenario: Bins below the minimum
- **WHEN** `analyze` is called at 48 kHz, FFT size 8192, `min_freq=20`
- **THEN** `output[0..=3]` are 0 (bin 3 is 17.58 Hz)

#### Scenario: Range narrowed after a wider call
- **WHEN** `analyze` is called with `(0, 24000)` repeatedly and then with `(100, 200)`
- **THEN** every entry outside bins 18 to 34 is 0 immediately after the narrower call

### Requirement: Range bounds are inclusive on bin frequency
A bin SHALL be in range when `min_freq <= i * (sample_rate / fft_size) <= max_freq`, evaluated in f32 with both bounds inclusive.

#### Scenario: Bin exactly on the minimum
- **WHEN** `analyze` is called at 48 kHz, FFT size 8192 with `min_freq=750`
- **THEN** bin 128 (750.0 Hz) is in range and `output[128]` may be non-zero

### Requirement: Smoothing is held per bin
Smoothing (0.9 previous, 0.1 new) SHALL be applied per absolute bin. A change of `min_freq` or `max_freq` between calls SHALL NOT alter the smoothed value of a bin that stays in range.

#### Scenario: Minimum changes between calls
- **WHEN** a 440 Hz tone is analyzed to convergence with `min_freq=0`, then analyzed with `min_freq=20`
- **THEN** the peak stays at index 75 and its value continues from the previous smoothed value rather than restarting

### Requirement: Bin re-entering the range has defined history
A bin that leaves and later re-enters the range SHALL start smoothing from a history value fixed by design (see design.md), identical on every such re-entry.

#### Scenario: Bin leaves and returns
- **WHEN** a bin is out of range for one or more calls and then in range again
- **THEN** its first in-range output is the same function of its new magnitude each time, independent of values held before it left

### Requirement: Buffer shape and pointer are stable
The buffer SHALL have length `fft_size/2` and SHALL be allocated once at construction and never reallocated. The constructor and `analyze` names and signatures SHALL be unchanged.

#### Scenario: Pointer stability
- **WHEN** `output()` is read before and after several `analyze` calls with different ranges
- **THEN** the returned pointer is identical

### Requirement: Pointer accessor is exported as `output`
The method returning the `*const f32` to the buffer SHALL be exported to JS as `output` with signature `() -> number`. No export named `ouput` SHALL exist.

#### Scenario: Generated bindings
- **WHEN** `wasm-pack build` is run and the generated `.d.ts` is read
- **THEN** it declares `output(): number` and does not declare `ouput`

#### Scenario: Same pointer as before the rename
- **WHEN** `output()` is called on a new analyzer
- **THEN** it returns the address of the first of `fft_size/2` contiguous f32 values, as `ouput()` did

### Requirement: Boundary validation is preserved
`analyze` SHALL still reject input shorter than `fft_size` samples, SHALL still use only the first `fft_size` samples of longer input, and SHALL still fail on invalid frequency limits.

#### Scenario: Short input
- **WHEN** `analyze` receives fewer than `fft_size` samples
- **THEN** it fails with a message naming both lengths

#### Scenario: Long input
- **WHEN** `analyze` receives more than `fft_size` samples
- **THEN** it succeeds using the first `fft_size` samples

## Related Specs

_None yet._
