use super::AmpModel;
use crate::amp::components::{
    cathode_follower, el34_bank, triode_stage, EnvelopeFollower, OnePoleLowpass, TopBoostToneStack,
    WdfHighpass,
};
use crate::amp::AmpControls;

pub(in crate::amp) struct Jcm800 {
    sample_rate: f32,
    input_coupling: WdfHighpass,
    cathode_bypass: WdfHighpass,
    bright_filter: OnePoleLowpass,
    tone_stack: TopBoostToneStack,
    phase_inverter_coupling: WdfHighpass,
    presence_filter: OnePoleLowpass,
    transformer_highpass: WdfHighpass,
    transformer_lowpass: OnePoleLowpass,
    bias_envelope: EnvelopeFollower,
    supply_sag: EnvelopeFollower,
}

impl Jcm800 {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            input_coupling: WdfHighpass::from_rc(sample_rate, 1_000_000.0, 2.2e-9),
            cathode_bypass: WdfHighpass::from_rc(sample_rate, 2_700.0, 680e-9),
            bright_filter: OnePoleLowpass::new(sample_rate, 3_800.0),
            tone_stack: TopBoostToneStack::new_with_caps(sample_rate, 470e-12, 22e-9, 22e-9),
            phase_inverter_coupling: WdfHighpass::from_rc(sample_rate, 1_000_000.0, 22e-9),
            presence_filter: OnePoleLowpass::new(sample_rate, 8_000.0),
            transformer_highpass: WdfHighpass::from_rc(sample_rate, 100_000.0, 22e-9),
            transformer_lowpass: OnePoleLowpass::new(sample_rate, 9_500.0),
            bias_envelope: EnvelopeFollower::new(sample_rate, 0.010, 0.180),
            supply_sag: EnvelopeFollower::new(sample_rate, 0.020, 0.320),
        }
    }
}

impl AmpModel for Jcm800 {
    fn reset(&mut self) {
        *self = Self::new(self.sample_rate);
    }

    #[inline]
    fn process(&mut self, input: f32, controls: AmpControls) -> f32 {
        let input = self.input_coupling.process(input);

        let preamp = controls.volume * controls.volume;
        let high = input - self.bright_filter.process(input);
        let high_sensitivity = input * (0.28 + preamp * 1.42) + high * (1.0 - preamp) * 0.30;
        let first_bypass = self.cathode_bypass.process(high_sensitivity);

        let first_stage = triode_stage(high_sensitivity * 5.8 + first_bypass * 1.6, 0.10);
        let cold_clipper_drive = first_stage * (3.2 + controls.drive * 4.4);
        let cold_clipper = triode_stage(cold_clipper_drive, -0.22);
        let recovery = triode_stage(cold_clipper * 2.6, 0.035);

        let cathode_follower = cathode_follower(recovery * 0.92);
        let middle = controls.cut.clamp(0.0, 1.0);
        let mid_scoop = 0.58 + middle * 0.52;
        let toned =
            self.tone_stack
                .process(cathode_follower * mid_scoop, controls.bass, controls.treble);

        let pi_input = self.phase_inverter_coupling.process(toned * 4.1);
        let phase_a = triode_stage(pi_input * 1.46, 0.025);
        let phase_b = triode_stage(-pi_input * 1.42, -0.020);
        let differential = (phase_a - phase_b) * 0.5;

        let presence_hz = 2_200.0 + controls.presence * 7_500.0;
        self.presence_filter
            .set_cutoff(self.sample_rate, presence_hz);
        let presence_low = self.presence_filter.process(differential);
        let presence = presence_low + (differential - presence_low) * (0.35 + controls.presence);

        let current_demand = (presence.abs() * 1.95 - 0.54).max(0.0);
        let bias_shift = self.bias_envelope.process(current_demand);
        let sag = self.supply_sag.process(current_demand * current_demand);
        let power_drive = presence * 1.95 / (1.0 + bias_shift * 0.35 + sag * 0.18);
        let positive_bank = el34_bank(power_drive - bias_shift * 0.025);
        let negative_bank = el34_bank(-power_drive - bias_shift * 0.022);
        let power_output = (positive_bank - negative_bank) * 0.82;

        let mut transformer = self.transformer_highpass.process(power_output);
        transformer = self.transformer_lowpass.process(transformer);
        transformer * controls.output * (1.0 - controls.sag * 0.10)
    }
}
