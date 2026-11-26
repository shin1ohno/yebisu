//! G3 proxy (not G3): per-snapshot adoption and GPUI layout + paint time on
//! the host's test platform. No line, no GPU rasterisation, no iPad.
//! `cargo test --release -p yebisu g3_proxy -- --ignored --nocapture`
use super::draw_tests::host;
use super::fixtures::Session;
use super::*;
use gpui::TestAppContext;
use std::time::{Duration, Instant};

const RUNS: usize = 300;

fn percentile(sorted: &[Duration], p: usize) -> Duration {
    let index = (sorted.len().saturating_sub(1) * p) / 100;
    sorted.get(index).copied().unwrap_or_default()
}

fn summary(mut samples: Vec<Duration>) -> String {
    samples.sort();
    format!(
        "p50={:.1}us p95={:.1}us max={:.1}us",
        percentile(&samples, 50).as_secs_f64() * 1e6,
        percentile(&samples, 95).as_secs_f64() * 1e6,
        samples.last().copied().unwrap_or_default().as_secs_f64() * 1e6,
    )
}

#[gpui::test]
#[ignore = "G3 proxy benchmark: run in release with --ignored --nocapture"]
fn g3_proxy_adopt_and_draw_per_snapshot(cx: &mut TestAppContext) {
    for fixture in Fixture::ALL {
        let mut adopt = Vec::with_capacity(RUNS);
        let mut bytes_len = 0;
        for _ in 0..RUNS {
            let mut session = Session::new().unwrap_or_else(|e| panic!("{e}"));
            let publish = fixture.publish(1).unwrap_or_else(|e| panic!("{e}"));
            let bytes = session.encode(publish).unwrap_or_else(|e| panic!("{e}"));
            bytes_len = bytes.len();
            let start = Instant::now();
            let scene = session.adopt_bytes(&bytes);
            adopt.push(start.elapsed());
            assert!(scene.is_ok(), "{scene:?}");
        }
        let (view, vcx) = host(cx, fixture);
        let scene = fixture.adopt().unwrap_or_else(|e| panic!("{e}"));
        let mut draw = Vec::with_capacity(RUNS);
        let mut total = Vec::with_capacity(RUNS);
        for run in 0..RUNS {
            let mut next = scene.clone();
            next.pub_rev = run as u64 + 2;
            let begin = Instant::now();
            view.update(vcx, |host, cx| {
                host.state.install(Ok(next));
                cx.notify();
            });
            // One full window draw: request_layout, prepaint and paint (the
            // test platform records the scene; there is no GPU).
            draw.push(vcx.update(|window, cx| {
                window.refresh();
                let start = Instant::now();
                window.draw(cx).clear(cx);
                start.elapsed()
            }));
            total.push(begin.elapsed());
        }
        println!(
            "G3-proxy fixture={} nodes={} bytes={bytes_len} adopt[{}] draw[{}] install+draw[{}]",
            fixture.name(),
            scene.nodes.len(),
            summary(adopt),
            summary(draw),
            summary(total),
        );
    }
}
