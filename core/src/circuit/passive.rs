#[derive(Clone, Copy)]
pub struct BrightVolumeInputParams {
    pub sample_rate: f32,
    pub input_resistance: f32,
    pub input_coupling_capacitance: f32,
    pub bright_cutoff_hz: f32,
    pub bright_bypass_gain: f32,
}

pub struct BrightVolumeInputStage {
    params: BrightVolumeInputParams,
    input_lowpass: OnePole,
    bright_lowpass: OnePole,
}

impl BrightVolumeInputStage {
    pub fn new(params: BrightVolumeInputParams) -> Self {
        let input_cutoff = 1.0
            / (std::f32::consts::TAU * params.input_resistance * params.input_coupling_capacitance);
        Self {
            params,
            input_lowpass: OnePole::new(params.sample_rate, input_cutoff),
            bright_lowpass: OnePole::new(params.sample_rate, params.bright_cutoff_hz),
        }
    }

    pub fn reset(&mut self) {
        self.input_lowpass.reset();
        self.bright_lowpass.reset();
    }

    pub fn process(&mut self, input: f32, volume: f32) -> f32 {
        let coupled = input - self.input_lowpass.process(input);
        let volume = volume.clamp(0.0, 1.0);
        let volume_gain = volume * volume;
        let bright = coupled - self.bright_lowpass.process(coupled);

        coupled * volume_gain + bright * (1.0 - volume_gain) * self.params.bright_bypass_gain
    }
}

struct OnePole {
    coefficient: f32,
    state: f32,
}

impl OnePole {
    fn new(sample_rate: f32, cutoff_hz: f32) -> Self {
        Self {
            coefficient: 1.0 - (-std::f32::consts::TAU * cutoff_hz / sample_rate).exp(),
            state: 0.0,
        }
    }

    fn reset(&mut self) {
        self.state = 0.0;
    }

    fn process(&mut self, input: f32) -> f32 {
        self.state += self.coefficient * (input - self.state);
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage() -> BrightVolumeInputStage {
        BrightVolumeInputStage::new(BrightVolumeInputParams {
            sample_rate: 48_000.0,
            input_resistance: 1_000_000.0,
            input_coupling_capacitance: 47e-9,
            bright_cutoff_hz: 2_900.0,
            bright_bypass_gain: 0.18,
        })
    }

    #[test]
    fn input_coupling_blocks_dc() {
        let mut stage = stage();
        let mut sum = 0.0;
        for sample_idx in 0..96_000 {
            let output = stage.process(0.4, 1.0);
            if sample_idx >= 95_000 {
                sum += output.abs();
            }
        }

        assert!(sum / 1_000.0 < 0.01, "settled_dc={}", sum / 1_000.0);
    }

    #[test]
    fn volume_reduces_midband_level() {
        let mut open = stage();
        let mut low = stage();
        let open_rms = sine_rms(&mut open, 1_000.0, 0.1, 1.0);
        let low_rms = sine_rms(&mut low, 1_000.0, 0.1, 0.35);

        assert!(open_rms > low_rms * 5.0, "open={open_rms}, low={low_rms}");
    }

    #[test]
    fn bright_path_keeps_highs_when_volume_is_low() {
        let mut low_frequency = stage();
        let mut high_frequency = stage();
        let low_rms = sine_rms(&mut low_frequency, 300.0, 0.1, 0.15);
        let high_rms = sine_rms(&mut high_frequency, 5_000.0, 0.1, 0.15);

        assert!(
            high_rms > low_rms * 1.8,
            "low_rms={low_rms}, high_rms={high_rms}"
        );
    }

    #[test]
    fn reset_clears_filter_history() {
        let mut stage = stage();
        for _ in 0..24_000 {
            stage.process(0.3, 0.8);
        }
        stage.reset();
        let first = stage.process(0.0, 0.8);

        assert!(first.abs() < 1e-6, "first={first}");
    }

    fn sine_rms(
        stage: &mut BrightVolumeInputStage,
        frequency: f32,
        amplitude: f32,
        volume: f32,
    ) -> f32 {
        let mut sum = 0.0;
        let mut count = 0;
        for sample_idx in 0..48_000 {
            let input = (std::f32::consts::TAU * frequency * sample_idx as f32 / 48_000.0).sin()
                * amplitude;
            let output = stage.process(input, volume);
            if sample_idx >= 24_000 {
                sum += output * output;
                count += 1;
            }
        }
        (sum / count as f32).sqrt()
    }
}
