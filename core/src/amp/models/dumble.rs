use super::AmpModel;
use crate::amp::components::{
    cathode_follower, el84_bank, triode_stage, EnvelopeFollower, OnePoleLowpass, TopBoostToneStack,
    WdfHighpass,
};
use crate::amp::AmpControls;

pub(in crate::amp) struct Dumble {
    sample_rate: f32,
    input_coupling: WdfHighpass,
    first_cathode_bypass: WdfHighpass,
    bright_filter: OnePoleLowpass,
    tone_stack: TopBoostToneStack,
    phase_inverter_coupling: WdfHighpass,
    presence_filter: OnePoleLowpass,
    transformer_highpass: WdfHighpass,
    transformer_lowpass: OnePoleLowpass,
    bias_envelope: EnvelopeFollower,
    supply_sag: EnvelopeFollower,
}

impl Dumble {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            input_coupling: WdfHighpass::from_rc(sample_rate, 1_000_000.0, 68e-12),
            first_cathode_bypass: WdfHighpass::from_rc(sample_rate, 2_200.0, 25e-6),
            bright_filter: OnePoleLowpass::new(sample_rate, 3_500.0),
            tone_stack: TopBoostToneStack::new_with_caps(sample_rate, 220e-12, 2.2e-9, 22e-9),
            phase_inverter_coupling: WdfHighpass::from_rc(sample_rate, 1_000_000.0, 47e-9),
            presence_filter: OnePoleLowpass::new(sample_rate, 10_000.0),
            transformer_highpass: WdfHighpass::from_rc(sample_rate, 100_000.0, 47e-9),
            transformer_lowpass: OnePoleLowpass::new(sample_rate, 14_000.0),
            bias_envelope: EnvelopeFollower::new(sample_rate, 0.006, 0.140),
            supply_sag: EnvelopeFollower::new(sample_rate, 0.018, 0.300),
        }
    }
}

impl AmpModel for Dumble {
    fn reset(&mut self) {
        *self = Self::new(self.sample_rate);
    }

    #[inline]
    fn process(&mut self, input: f32, controls: AmpControls) -> f32 {
        let input = self.input_coupling.process(input);

        let volume = controls.volume * controls.volume;
        let high = input - self.bright_filter.process(input);
        let volume_output = input * volume + high * (1.0 - volume) * 0.18;

        let first_bypass = self.first_cathode_bypass.process(volume_output);
        let mut first_drive = volume_output * 4.8 + first_bypass * 0.8;
        first_drive *= 1.0 + controls.drive * 0.6;
        let first_stage = triode_stage(first_drive, 0.16);

        let cathode_follower = cathode_follower(first_stage);
        let toned = self
            .tone_stack
            .process(cathode_follower, controls.bass, controls.treble);

        let pi_input = self.phase_inverter_coupling.process(toned * 4.8);
        let phase_a = triode_stage(pi_input * 1.38, 0.045);
        let phase_b = triode_stage(-pi_input * 1.32, -0.035);
        let differential = (phase_a - phase_b) * 0.5;

        let cut_hz = 13_500.0 * (1.0 - controls.cut).powi(2) + 1_150.0;
        self.presence_filter.set_cutoff(self.sample_rate, cut_hz);
        let cut_output = self.presence_filter.process(differential);

        let current_demand = (cut_output.abs() * 1.60 - 0.62).max(0.0);
        let bias_shift = self.bias_envelope.process(current_demand);
        let sag = self.supply_sag.process(current_demand * current_demand);
        let drive = cut_output * 1.60 / (1.0 + bias_shift * 0.55 + sag * 0.22);
        let positive_bank = el84_bank(drive - bias_shift * 0.055);
        let negative_bank = el84_bank(-drive - bias_shift * 0.045);
        let power_output = (positive_bank - negative_bank) * 0.72;

        let mut transformer = self.transformer_highpass.process(power_output);
        transformer = self.transformer_lowpass.process(transformer);

        let mut out = transformer * controls.output;
        out *= 1.0 + controls.presence * 0.03;
        out *= 1.0 - controls.sag * 0.12;
        out
    }
}
