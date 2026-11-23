use spectrum_analyzer::scaling::SpectrumDataStats;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
use std::f32::consts::PI;
use wasm_bindgen::prelude::*;
mod utils;

#[wasm_bindgen(getter_with_clone)]
pub struct WasmSpectrumAnalyzer {
    visualize_spectrum: Vec<(f32, f32)>,
    sample_rate: u32,
    fft_size: usize,
    output: Vec<f32>,
    hann_coeffs: Vec<f32>,
    hann_buf: Vec<f32>,
    initialized: bool,
}

fn normalize(fr_val: f32, _stats: &SpectrumDataStats) -> f32 {
    debug_assert!(!fr_val.is_infinite());
    debug_assert!(!fr_val.is_nan());
    debug_assert!(fr_val >= 0.0);

    // if stats.n == 0.0 {
    //     20.0 * libm::log10f(2.0 * &fr_val)
    // } else {
    //     20.0 * libm::log10f(2.0 * &fr_val / stats.n)
    // }
    fr_val
}

#[wasm_bindgen]
impl WasmSpectrumAnalyzer {
    #[wasm_bindgen(constructor)]
    pub fn new(sample_rate: u32, fft_size: usize) -> Self {
        utils::set_panic_hook();

        let hann_coeffs: Vec<f32> = (0..fft_size)
            .map(|i| {
                let two_pi_i = 2.0 * PI * i as f32;
                let idm1 = (fft_size - 1) as f32;
                0.5 * (1.0 - (two_pi_i / idm1).cos())
            })
            .collect();

        WasmSpectrumAnalyzer {
            sample_rate,
            fft_size,
            visualize_spectrum: vec![(0.0_f32, 0.0_f32); fft_size / 2],
            output: vec![0.0; fft_size / 2],
            hann_buf: vec![0.0; fft_size],
            hann_coeffs,
            initialized: false,
        }
    }

    pub fn analyze(&mut self, audio_samples: &[f32], min_freq: u32, max_freq: u32) {
        if audio_samples.len() < self.fft_size {
            panic!("Insufficient samples passed to analyze(). Expected an array containing {} elements but got {}", self.fft_size, audio_samples.len());
        }
        // Apply Hann window into pre-allocated buffer — no heap allocation
        self.hann_buf
            .iter_mut()
            .zip(audio_samples[..self.fft_size].iter())
            .zip(self.hann_coeffs.iter())
            .for_each(|((buf, sample), coeff)| *buf = sample * coeff);

        let latest_spectrum = samples_fft_to_spectrum(
            &self.hann_buf,
            self.sample_rate,
            FrequencyLimit::Range(min_freq as f32, max_freq as f32),
            Some(&normalize),
        )
        .unwrap();

        // Copy `initialized` flag so the closure doesn't borrow `self`
        let initialized = self.initialized;
        let half = self.fft_size / 2;
        let bin_hz = self.sample_rate as f32 / self.fft_size as f32;
        // Contiguous range of absolute bins returned by the FFT: lo..hi
        let mut lo = half;
        let mut hi = 0;
        for (fr_new, fr_val_new) in latest_spectrum.data().iter() {
            let bin = (fr_new.val() / bin_hz).round() as usize;
            if bin >= half {
                continue;
            }
            let (fr_old, fr_val_old) = &mut self.visualize_spectrum[bin];
            if !initialized {
                *fr_old = fr_new.val();
            }
            *fr_val_old = *fr_val_old * 0.9 + fr_val_new.val() * 0.1;
            lo = lo.min(bin);
            hi = hi.max(bin + 1);
        }
        self.initialized = true;

        // Out-of-range bins are zero (no smoothing, no decay)
        let hi = hi.max(lo);
        for (_, value) in self.visualize_spectrum[..lo].iter_mut() {
            *value = 0.0;
        }
        for (_, value) in self.visualize_spectrum[hi..].iter_mut() {
            *value = 0.0;
        }
        self.output[..lo].fill(0.0);
        self.output[hi..].fill(0.0);
        self.output[lo..hi]
            .iter_mut()
            .zip(self.visualize_spectrum[lo..hi].iter())
            .for_each(|(out, &(_, value))| *out = value);
    }

    pub fn output(&self) -> *const f32 {
        self.output.as_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONVERGE_CALLS: usize = 30;

    fn sine_wave(freq: f32, sample_rate: u32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| (2.0 * PI * freq * i as f32 / sample_rate as f32).sin())
            .collect()
    }

    fn peak_index(values: &[f32]) -> usize {
        values
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap()
    }

    #[test]
    fn peak_is_at_true_bin_with_nonzero_min_freq() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);

        for _ in 0..CONVERGE_CALLS {
            analyzer.analyze(&samples, 20, 20000);
        }

        assert_eq!(peak_index(&analyzer.output), 75);
    }

    #[test]
    fn peak_stays_at_true_bin_when_min_freq_changes() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);
        for _ in 0..CONVERGE_CALLS {
            analyzer.analyze(&samples, 0, 20000);
        }
        assert_eq!(peak_index(&analyzer.output), 75);

        for _ in 0..CONVERGE_CALLS {
            analyzer.analyze(&samples, 20, 20000);
        }

        assert_eq!(peak_index(&analyzer.output), 75);
    }

    #[test]
    fn min_freq_zero_peak_is_at_true_bin() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);

        for _ in 0..CONVERGE_CALLS {
            analyzer.analyze(&samples, 0, 20000);
        }

        assert_eq!(peak_index(&analyzer.output), 75);
    }

    #[test]
    fn bins_below_min_freq_are_zero() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);

        analyzer.analyze(&samples, 20, 20000);

        assert_eq!(analyzer.output[0..=3], [0.0; 4]);
    }

    #[test]
    fn narrowing_range_zeroes_every_bin_outside_it() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);
        for _ in 0..CONVERGE_CALLS {
            analyzer.analyze(&samples, 0, 24000);
        }

        analyzer.analyze(&samples, 100, 200);

        let nonzero_outside = analyzer
            .output
            .iter()
            .enumerate()
            .filter(|&(i, &v)| !(18..=34).contains(&i) && v != 0.0)
            .count();
        assert_eq!(nonzero_outside, 0);
        assert_eq!(analyzer.visualize_spectrum[17].1, 0.0);
        assert_eq!(analyzer.visualize_spectrum[35].1, 0.0);
    }

    #[test]
    fn min_freq_on_bin_frequency_is_inclusive() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);

        analyzer.analyze(&samples, 750, 20000);

        assert_eq!(analyzer.output.iter().position(|&v| v != 0.0), Some(128));
    }

    #[test]
    fn reentering_bin_starts_from_fresh_history() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let tone = sine_wave(440.0, 48000, 8192);
        let other = sine_wave(1000.0, 48000, 8192);
        for _ in 0..CONVERGE_CALLS {
            analyzer.analyze(&tone, 0, 20000);
        }
        analyzer.analyze(&tone, 1000, 2000);
        let mut fresh = WasmSpectrumAnalyzer::new(48000, 8192);

        analyzer.analyze(&other, 0, 20000);
        fresh.analyze(&other, 0, 20000);

        assert_eq!(analyzer.output[75], fresh.output[75]);
    }

    #[test]
    fn first_call_output_is_a_tenth_of_raw_magnitude() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);

        analyzer.analyze(&samples, 0, 20000);

        let raw = samples_fft_to_spectrum(
            &analyzer.hann_buf,
            48000,
            FrequencyLimit::Range(0.0, 20000.0),
            Some(&normalize),
        )
        .unwrap();
        let raw_bin_75 = raw
            .data()
            .iter()
            .find(|(f, _)| (f.val() / (48000.0 / 8192.0)).round() as usize == 75)
            .map(|(_, v)| v.val())
            .unwrap();
        assert_eq!(analyzer.output[75], raw_bin_75 * 0.1);
    }

    #[test]
    fn output_pointer_is_stable_and_matches_buffer() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);
        let before = analyzer.output();

        analyzer.analyze(&samples, 0, 20000);
        analyzer.analyze(&samples, 100, 200);

        assert_eq!(analyzer.output(), before);
        assert_eq!(analyzer.output(), analyzer.output.as_ptr());
    }

    #[test]
    fn output_length_is_half_fft_size() {
        let analyzer = WasmSpectrumAnalyzer::new(48000, 8192);

        assert_eq!(analyzer.output.len(), 4096);
    }

    #[test]
    fn peak_is_at_true_bin_at_44100_hz_and_1024_fft() {
        let mut analyzer = WasmSpectrumAnalyzer::new(44100, 1024);
        let samples = sine_wave(440.0, 44100, 1024);

        for _ in 0..CONVERGE_CALLS {
            analyzer.analyze(&samples, 20, 20000);
        }

        assert_eq!(peak_index(&analyzer.output), 10);
        assert_eq!(analyzer.output[0], 0.0);
    }

    #[test]
    fn peak_is_at_true_bin_at_96000_hz() {
        let mut analyzer = WasmSpectrumAnalyzer::new(96000, 8192);
        let samples = sine_wave(440.0, 96000, 8192);

        for _ in 0..CONVERGE_CALLS {
            analyzer.analyze(&samples, 20, 20000);
        }

        assert_eq!(peak_index(&analyzer.output), 38);
    }

    #[test]
    #[should_panic(expected = "Expected an array containing 8192 elements but got 100")]
    fn analyze_panics_on_short_input() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);

        analyzer.analyze(&[0.0; 100], 20, 20000);
    }

    #[test]
    #[should_panic]
    fn analyze_panics_when_min_freq_exceeds_max_freq() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);

        analyzer.analyze(&samples, 2000, 1000);
    }

    #[test]
    #[should_panic]
    fn analyze_panics_when_max_freq_exceeds_nyquist() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);

        analyzer.analyze(&samples, 20, 24001);
    }

    #[test]
    #[should_panic]
    fn analyze_panics_when_range_holds_fewer_than_two_bins() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);

        analyzer.analyze(&samples, 100, 102);
    }

    #[test]
    fn analyze_uses_first_fft_size_samples_of_longer_input() {
        let mut exact = WasmSpectrumAnalyzer::new(48000, 8192);
        let mut longer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 10000);

        exact.analyze(&samples[..8192], 20, 20000);
        longer.analyze(&samples, 20, 20000);

        assert_eq!(longer.output, exact.output);
    }

    #[test]
    fn panicking_call_leaves_output_unchanged() {
        let mut analyzer = WasmSpectrumAnalyzer::new(48000, 8192);
        let samples = sine_wave(440.0, 48000, 8192);
        analyzer.analyze(&samples, 20, 20000);
        let before = analyzer.output.clone();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            analyzer.analyze(&samples, 2000, 1000);
        }));

        assert!(result.is_err());
        assert_eq!(analyzer.output, before);
    }
}
