use super::AmpModel;
use crate::amp::components::{
    six_l6_bank, triode_stage, EnvelopeFollower, OnePoleLowpass, WdfHighpass,
};
use crate::amp::AmpControls;

pub(in crate::amp) struct Dumble {
    sample_rate: f32,
    input_coupling: WdfHighpass,
    first_cathode_bypass: WdfHighpass,
    clean_volume_coupling: WdfHighpass,
    drive_cathode_bypass: WdfHighpass,
    stack_low_filter: OnePoleLowpass,
    stack_high_filter: OnePoleLowpass,
    overdrive_coupling: WdfHighpass,
    overdrive_trim_filter: OnePoleLowpass,
    recovery_coupling: WdfHighpass,
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
            first_cathode_bypass: WdfHighpass::from_rc(sample_rate, 1_500.0, 5e-6),
            clean_volume_coupling: WdfHighpass::from_rc(sample_rate, 250_000.0, 22e-9),
            drive_cathode_bypass: WdfHighpass::from_rc(sample_rate, 2_200.0, 25e-6),
            stack_low_filter: OnePoleLowpass::new(sample_rate, 260.0),
            stack_high_filter: OnePoleLowpass::new(sample_rate, 2_400.0),
            overdrive_coupling: WdfHighpass::from_rc(sample_rate, 220_000.0, 4.7e-9),
            overdrive_trim_filter: OnePoleLowpass::new(sample_rate, 3_600.0),
            recovery_coupling: WdfHighpass::from_rc(sample_rate, 220_000.0, 47e-9),
            phase_inverter_coupling: WdfHighpass::from_rc(sample_rate, 1_000_000.0, 47e-9),
            presence_filter: OnePoleLowpass::new(sample_rate, 3_200.0),
            transformer_highpass: WdfHighpass::from_rc(sample_rate, 100_000.0, 47e-9),
            transformer_lowpass: OnePoleLowpass::new(sample_rate, 11_500.0),
            bias_envelope: EnvelopeFollower::new(sample_rate, 0.014, 0.220),
            supply_sag: EnvelopeFollower::new(sample_rate, 0.026, 0.420),
        }
    }

    #[inline]
    fn tone_stack(&mut self, input: f32, bass: f32, mid: f32, treble: f32) -> f32 {
        let low = self.stack_low_filter.process(input);
        let high = input - self.stack_high_filter.process(input);
        let mid_band = input - low - high;

        let bass_gain = 0.45 + bass.clamp(0.0, 1.0) * 0.85;
        let mid_gain = 0.35 + mid.clamp(0.0, 1.0) * 0.75;
        let treble_gain = 0.40 + treble.clamp(0.0, 1.0) * 0.95;

        (low * bass_gain + mid_band * mid_gain + high * treble_gain) * 0.58
    }
}

impl AmpModel for Dumble {
    fn reset(&mut self) {
        *self = Self::new(self.sample_rate);
    }

    #[inline]
    fn process(&mut self, input: f32, controls: AmpControls) -> f32 {
        let input = self.input_coupling.process(input);

        let first_bypass = self.first_cathode_bypass.process(input);
        let first_stage = triode_stage(input * 3.4 + first_bypass * 0.55, 0.085);

        let clean_volume = controls.volume * controls.volume;
        let clean_feed = self
            .clean_volume_coupling
            .process(first_stage * (0.24 + clean_volume * 1.35));

        let drive_bypass = self.drive_cathode_bypass.process(clean_feed);
        let second_stage = triode_stage(clean_feed * 3.0 + drive_bypass * 0.9, 0.045);

        let stacked = self.tone_stack(second_stage, controls.bass, controls.cut, controls.treble);

        let overdrive_amount = controls.drive.clamp(0.0, 1.0);
        let od_input = self
            .overdrive_coupling
            .process(stacked * (0.55 + overdrive_amount * 2.9));
        let od_first = triode_stage(od_input * (1.4 + overdrive_amount * 2.8), -0.040);
        let od_smoothed = self.overdrive_trim_filter.process(od_first);
        let od_second = triode_stage(od_smoothed * (1.2 + overdrive_amount * 2.2), 0.060);

        let clean_recovery = triode_stage(stacked * 2.2, 0.020);
        let lead_mix = overdrive_amount * overdrive_amount;
        let blended = clean_recovery * (1.0 - lead_mix * 0.72) + od_second * lead_mix;

        let recovery_input = self.recovery_coupling.process(blended);
        let recovery = triode_stage(recovery_input * 2.15, 0.030);

        let pi_input = self.phase_inverter_coupling.process(recovery * 3.6);
        let phase_a = triode_stage(pi_input * 1.24, 0.020);
        let phase_b = triode_stage(-pi_input * 1.20, -0.018);
        let differential = (phase_a - phase_b) * 0.5;

        let presence_hz = 1_800.0 + controls.presence * 5_800.0;
        self.presence_filter
            .set_cutoff(self.sample_rate, presence_hz);
        let feedback_low = self.presence_filter.process(differential);
        let presence = feedback_low + (differential - feedback_low) * (0.20 + controls.presence);

        let current_demand = (presence.abs() * 1.72 - 0.50).max(0.0);
        let bias_shift = self.bias_envelope.process(current_demand);
        let sag = self.supply_sag.process(current_demand * current_demand);
        let power_drive = presence * 1.68 / (1.0 + bias_shift * 0.32 + sag * 0.20);
        let positive_bank = six_l6_bank(power_drive - bias_shift * 0.022);
        let negative_bank = six_l6_bank(-power_drive - bias_shift * 0.020);
        let power_output = (positive_bank - negative_bank) * 0.80;

        let mut transformer = self.transformer_highpass.process(power_output);
        transformer = self.transformer_lowpass.process(transformer);
        transformer * controls.output * (1.0 - controls.sag * 0.10)
    }
}
