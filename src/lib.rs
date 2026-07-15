use std::f32::consts::PI;
use spectrum_analyzer::scaling::SpectrumDataStats;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
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
        latest_spectrum
            .data()
            .iter()
            .zip(self.visualize_spectrum.iter_mut())
            .for_each(|((fr_new, fr_val_new), (fr_old, fr_val_old))| {
                if !initialized {
                    *fr_old = fr_new.val();
                }

                let old_val = *fr_val_old * 0.9;
                let new_val = fr_val_new.val() * 0.1;
                *fr_val_old = old_val + new_val;
            });
        self.initialized = true;

        self.output
            .iter_mut()
            .zip(self.visualize_spectrum.iter())
            .for_each(|(out, &(_, value))| *out = value);
    }

    pub fn ouput(&self) -> *const f32 {
        self.output.as_ptr()
    }
}
