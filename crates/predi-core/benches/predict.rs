//! Criterion micro-benchmarks for `predict` on the fixture asset (T059, research R11).

#[cfg(not(target_arch = "wasm32"))]
mod bench {
    use criterion::{Criterion, criterion_group};
    use predi_core::{Engine, PredictionRequest};
    use std::hint::black_box;

    const FIXTURE: &[u8] = include_bytes!("../fixtures/mini.pta");

    fn predict(c: &mut Criterion) {
        let engine = match Engine::from_bytes(FIXTURE) {
            Ok(e) => e,
            Err(e) => panic!("fixture: {e}"),
        };
        let long_boundary = format!("{}Muchas gracias por ", "Buenos días, gracias. ".repeat(50));
        let inputs: [(&str, &str); 5] = [
            ("empty", ""),
            ("short_partial", "Buenos d"),
            ("long_partial", "Le pedimos una disculpa por las molest"),
            ("boundary_after_1000_chars", &long_boundary),
            ("not_applicable", "Escríbanos a soporte@emp"),
        ];
        for (name, text) in inputs {
            c.bench_function(name, |b| {
                b.iter(|| engine.predict(black_box(&PredictionRequest::new(1, text))))
            });
        }
        c.bench_function("load_fixture", |b| {
            b.iter(|| Engine::from_bytes(black_box(FIXTURE)))
        });
    }

    criterion_group!(benches, predict);
}

#[cfg(not(target_arch = "wasm32"))]
criterion::criterion_main!(bench::benches);

#[cfg(target_arch = "wasm32")]
fn main() {}
