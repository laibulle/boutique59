use greybound::amp::{AmpControls, VoxAmp};
use std::time::Instant;

fn main() {
    let model = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "nox30".to_string());
    let sample_rate = 44_100.0;
    let seconds = 10;
    let mut amp = VoxAmp::with_model(sample_rate, &model);
    let controls = AmpControls {
        volume: 0.76,
        bass: 0.52,
        treble: 0.61,
        cut: 0.47,
        output: 10.0_f32.powf(-18.0 / 20.0),
        drive: 0.68,
        presence: 0.44,
        sag: 0.70,
    };

    let sample_count = sample_rate as usize * seconds;
    let start = Instant::now();
    let mut sum = 0.0_f32;
    for sample_idx in 0..sample_count {
        let t = sample_idx as f32 / sample_rate;
        let chord = (std::f32::consts::TAU * 196.0 * t).sin()
            + (std::f32::consts::TAU * 247.0 * t).sin() * 0.7
            + (std::f32::consts::TAU * 330.0 * t).sin() * 0.45;
        let pick = if sample_idx % 1_571 < 80 { 1.35 } else { 1.0 };
        sum += amp.process(chord * 0.055 * pick, controls);
    }
    let elapsed = start.elapsed();
    let realtime = seconds as f64 / elapsed.as_secs_f64();

    println!(
        "{model}: processed {seconds}s in {:.3}s ({realtime:.1}x realtime), checksum={sum:.6}",
        elapsed.as_secs_f64()
    );
}
