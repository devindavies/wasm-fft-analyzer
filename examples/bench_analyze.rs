//! Regression guard for `WasmSpectrumAnalyzer::analyze`.
//! Run: `cargo run --release --example bench_analyze`
use std::alloc::{GlobalAlloc, Layout, System};
use std::f32::consts::PI;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;
use wasm_fft_analyzer::WasmSpectrumAnalyzer;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(layout.size(), Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(layout.size(), Relaxed);
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(new_size, Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static A: Counting = Counting;

const SAMPLE_RATE: u32 = 48_000;
const FFT_SIZE: usize = 8192;
const WARMUP: usize = 200;
const CALLS: usize = 2000;
const REPEATS: usize = 5;

fn median<T: PartialOrd + Copy>(v: &mut [T]) -> T {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let samples: Vec<f32> = (0..FFT_SIZE)
        .map(|i| (2.0 * PI * 440.0 * i as f32 / SAMPLE_RATE as f32).sin())
        .collect();
    let mut analyzer = WasmSpectrumAnalyzer::new(SAMPLE_RATE, FFT_SIZE);
    for _ in 0..WARMUP {
        analyzer.analyze(&samples, 20, 20000);
    }

    let (mut allocs, mut bytes, mut micros) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..REPEATS {
        let a0 = ALLOCS.load(Relaxed);
        let b0 = BYTES.load(Relaxed);
        let t = Instant::now();
        for _ in 0..CALLS {
            analyzer.analyze(&samples, 20, 20000);
        }
        let dt = t.elapsed();
        allocs.push((ALLOCS.load(Relaxed) - a0) as f64 / CALLS as f64);
        bytes.push((BYTES.load(Relaxed) - b0) as f64 / CALLS as f64);
        micros.push(dt.as_secs_f64() * 1e6 / CALLS as f64);
    }
    println!("allocations/call: {}", median(&mut allocs));
    println!("bytes/call:       {}", median(&mut bytes));
    println!("microseconds/call: {:.2}", median(&mut micros));
}
