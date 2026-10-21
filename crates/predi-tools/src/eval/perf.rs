//! Latency, memory and startup measurement (T055, research R11, SC-009–SC-011).

use super::report::Latency;
use predi_core::{Engine, PredictionRequest};
use std::time::Instant;

pub const WARMUP_REQUESTS: usize = 1000;

/// Current process working set in bytes (`GetProcessMemoryInfo` on Windows), if available.
pub fn working_set() -> Option<u64> {
    memory_stats::memory_stats().map(|m| m.physical_mem as u64)
}

/// Tracks the peak working set across samples.
#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryPeak {
    pub peak_bytes: u64,
    pub samples: u64,
}

impl MemoryPeak {
    pub fn sample(&mut self) {
        if let Some(b) = working_set() {
            self.peak_bytes = self.peak_bytes.max(b);
            self.samples += 1;
        }
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    // Nearest-rank percentile.
    let rank = (p / 100.0 * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

/// Time every request after a warm-up, sampling memory every 1,000 requests (and before/after).
pub fn measure(engine: &Engine, inputs: &[&str], memory: &mut MemoryPeak) -> Latency {
    memory.sample();
    for (i, text) in inputs
        .iter()
        .cycle()
        .take(WARMUP_REQUESTS.min(inputs.len() * 4))
        .enumerate()
    {
        let _ = engine.predict(&PredictionRequest::new(i as u64, text));
    }
    let mut ms: Vec<f64> = Vec::with_capacity(inputs.len());
    for (i, text) in inputs.iter().enumerate() {
        let req = PredictionRequest::new(i as u64, text);
        let t = Instant::now();
        let r = engine.predict(&req);
        let elapsed = t.elapsed();
        std::hint::black_box(r);
        ms.push(elapsed.as_secs_f64() * 1000.0);
        if i % 1000 == 0 {
            memory.sample();
        }
    }
    memory.sample();
    ms.sort_by(f64::total_cmp);
    Latency {
        p50: percentile(&ms, 50.0),
        p95: percentile(&ms, 95.0),
        p99: percentile(&ms, 99.0),
        max: ms.last().copied().unwrap_or(0.0),
        requests: ms.len() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&v, 50.0), 50.0);
        assert_eq!(percentile(&v, 95.0), 95.0);
        assert_eq!(percentile(&v, 99.0), 99.0);
        assert_eq!(percentile(&[], 99.0), 0.0);
    }
}
