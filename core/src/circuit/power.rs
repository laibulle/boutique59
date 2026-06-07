#[derive(Clone, Copy)]
pub struct PushPullEl84Params {
    pub sample_rate: f32,
    pub nominal_supply_voltage: f32,
    pub screen_voltage: f32,
    pub screen_resistance: f32,
    pub screen_capacitance: f32,
    pub primary_half_resistance: f32,
    pub supply_resistance: f32,
    pub supply_capacitance: f32,
    pub cathode_resistance: f32,
    pub cathode_capacitance: f32,
    pub idle_current: f32,
    pub drive_gain: f32,
    pub current_gain: f32,
    pub load_current_coupling: f32,
    pub attack_current_coupling: f32,
    pub compression: f32,
    pub output_scale: f32,
}

#[derive(Clone, Copy)]
pub struct OutputTransformerParams {
    pub sample_rate: f32,
    pub primary_resistance: f32,
    pub primary_inductance: f32,
    pub leakage_cutoff_hz: f32,
    pub core_saturation: f32,
    pub secondary_saturation_voltage: f32,
    pub output_scale: f32,
}

#[derive(Clone, Copy)]
pub struct SupplyNetworkParams {
    pub sample_rate: f32,
    pub rectifier_voltage: f32,
    pub power_nominal_voltage: f32,
    pub phase_inverter_nominal_voltage: f32,
    pub preamp_nominal_voltage: f32,
    pub rectifier_resistance: f32,
    pub phase_inverter_resistance: f32,
    pub preamp_resistance: f32,
    pub reservoir_capacitance: f32,
    pub phase_inverter_capacitance: f32,
    pub preamp_capacitance: f32,
}

pub struct PushPullEl84Stage {
    params: PushPullEl84Params,
    supply_voltage: f32,
    screen_voltage: f32,
    cathode_bias_voltage: f32,
    plate_a_voltage: f32,
    plate_b_voltage: f32,
    reference_plate_a_voltage: f32,
    reference_plate_b_voltage: f32,
    positive_current: f32,
    negative_current: f32,
    positive_load_current: f32,
    negative_load_current: f32,
    positive_screen_current: f32,
    negative_screen_current: f32,
    attack_current: f32,
    attack_reference_current: f32,
}

pub struct OutputTransformerStage {
    params: OutputTransformerParams,
    primary_lowpass: OnePole,
    leakage_lowpass: OnePole,
    core_flux: f32,
}

pub struct SupplyNetwork {
    params: SupplyNetworkParams,
    power_voltage: f32,
    phase_inverter_voltage: f32,
    preamp_voltage: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PushPullEl84OperatingPoint {
    pub supply_voltage: f32,
    pub screen_voltage: f32,
    pub plate_a_voltage: f32,
    pub plate_b_voltage: f32,
    pub cathode_bias_voltage: f32,
    pub positive_current: f32,
    pub negative_current: f32,
    pub positive_load_current: f32,
    pub negative_load_current: f32,
    pub positive_screen_current: f32,
    pub negative_screen_current: f32,
    pub attack_current: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct OutputTransformerOperatingPoint {
    pub core_flux: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SupplyNetworkOperatingPoint {
    pub power_voltage: f32,
    pub phase_inverter_voltage: f32,
    pub preamp_voltage: f32,
}

#[derive(Clone, Copy)]
struct PentodePoint {
    current: f32,
    screen_current: f32,
    d_current_d_plate: f32,
}

impl OutputTransformerStage {
    pub fn new(params: OutputTransformerParams) -> Self {
        let primary_cutoff_hz = params.primary_resistance
            / (std::f32::consts::TAU * params.primary_inductance.max(1e-6));
        Self {
            params,
            primary_lowpass: OnePole::new(params.sample_rate, primary_cutoff_hz),
            leakage_lowpass: OnePole::new(params.sample_rate, params.leakage_cutoff_hz),
            core_flux: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.primary_lowpass.reset();
        self.leakage_lowpass.reset();
        self.core_flux = 0.0;
    }

    pub fn operating_point(&self) -> OutputTransformerOperatingPoint {
        OutputTransformerOperatingPoint {
            core_flux: self.core_flux,
        }
    }

    pub fn process(&mut self, primary_voltage: f32) -> f32 {
        let primary_highpass = primary_voltage - self.primary_lowpass.process(primary_voltage);
        let flux_coefficient = 1.0 / self.params.sample_rate;
        self.core_flux += flux_coefficient * primary_highpass;
        let saturation = 1.0 / (1.0 + (self.core_flux.abs() * self.params.core_saturation).powi(2));
        self.core_flux *= 0.9995;

        let secondary = self
            .leakage_lowpass
            .process(primary_highpass * saturation * self.params.output_scale);
        let knee = self.params.secondary_saturation_voltage.max(1e-6);
        (secondary / knee).tanh() * knee
    }
}

impl SupplyNetwork {
    pub fn new(params: SupplyNetworkParams) -> Self {
        Self {
            params,
            power_voltage: params.power_nominal_voltage,
            phase_inverter_voltage: params.phase_inverter_nominal_voltage,
            preamp_voltage: params.preamp_nominal_voltage,
        }
    }

    pub fn reset(&mut self) {
        self.power_voltage = self.params.power_nominal_voltage;
        self.phase_inverter_voltage = self.params.phase_inverter_nominal_voltage;
        self.preamp_voltage = self.params.preamp_nominal_voltage;
    }

    pub fn operating_point(&self) -> SupplyNetworkOperatingPoint {
        SupplyNetworkOperatingPoint {
            power_voltage: self.power_voltage,
            phase_inverter_voltage: self.phase_inverter_voltage,
            preamp_voltage: self.preamp_voltage,
        }
    }

    pub fn process(
        &mut self,
        preamp_current: f32,
        phase_inverter_current: f32,
        power_current: f32,
        sag: f32,
    ) -> SupplyNetworkOperatingPoint {
        let sag = sag.clamp(0.0, 1.0);
        let power_current = power_current.max(0.0);
        let phase_inverter_current = phase_inverter_current.max(0.0);
        let preamp_current = preamp_current.max(0.0);
        let power_target = self.params.rectifier_voltage
            - power_current * self.params.rectifier_resistance * (0.30 + sag);
        let phase_inverter_target = power_target
            - (phase_inverter_current + preamp_current) * self.params.phase_inverter_resistance;
        let preamp_target = phase_inverter_target - preamp_current * self.params.preamp_resistance;

        self.power_voltage = smooth_voltage(
            self.power_voltage,
            power_target,
            self.params.sample_rate,
            self.params.rectifier_resistance,
            self.params.reservoir_capacitance,
        )
        .clamp(
            self.params.power_nominal_voltage * 0.55,
            self.params.rectifier_voltage,
        );
        self.phase_inverter_voltage = smooth_voltage(
            self.phase_inverter_voltage,
            phase_inverter_target,
            self.params.sample_rate,
            self.params.phase_inverter_resistance,
            self.params.phase_inverter_capacitance,
        )
        .clamp(
            self.params.phase_inverter_nominal_voltage * 0.55,
            self.power_voltage,
        );
        self.preamp_voltage = smooth_voltage(
            self.preamp_voltage,
            preamp_target,
            self.params.sample_rate,
            self.params.preamp_resistance,
            self.params.preamp_capacitance,
        )
        .clamp(
            self.params.preamp_nominal_voltage * 0.55,
            self.phase_inverter_voltage,
        );

        self.operating_point()
    }
}

impl PushPullEl84Stage {
    pub fn new(params: PushPullEl84Params) -> Self {
        let idle_cathode = params.idle_current * params.cathode_resistance;
        let idle_plate_drop = params.idle_current * 0.5 * params.primary_half_resistance;
        let idle_plate = params.nominal_supply_voltage - idle_plate_drop;
        let mut stage = Self {
            params,
            supply_voltage: params.nominal_supply_voltage,
            screen_voltage: params.screen_voltage,
            cathode_bias_voltage: idle_cathode,
            plate_a_voltage: idle_plate,
            plate_b_voltage: idle_plate,
            reference_plate_a_voltage: idle_plate,
            reference_plate_b_voltage: idle_plate,
            positive_current: params.idle_current * 0.5,
            negative_current: params.idle_current * 0.5,
            positive_load_current: 0.0,
            negative_load_current: 0.0,
            positive_screen_current: params.idle_current * 0.04,
            negative_screen_current: params.idle_current * 0.04,
            attack_current: 0.0,
            attack_reference_current: params.idle_current,
        };
        for _ in 0..512 {
            stage.process(0.0, 0.0);
        }
        stage.reference_plate_a_voltage = stage.plate_a_voltage;
        stage.reference_plate_b_voltage = stage.plate_b_voltage;
        stage
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.params);
    }

    pub fn operating_point(&self) -> PushPullEl84OperatingPoint {
        PushPullEl84OperatingPoint {
            supply_voltage: self.supply_voltage,
            screen_voltage: self.screen_voltage,
            plate_a_voltage: self.plate_a_voltage,
            plate_b_voltage: self.plate_b_voltage,
            cathode_bias_voltage: self.cathode_bias_voltage,
            positive_current: self.positive_current,
            negative_current: self.negative_current,
            positive_load_current: self.positive_load_current,
            negative_load_current: self.negative_load_current,
            positive_screen_current: self.positive_screen_current,
            negative_screen_current: self.negative_screen_current,
            attack_current: self.attack_current,
        }
    }

    pub fn process(&mut self, drive: f32, sag: f32) -> f32 {
        let supply_ratio = self.supply_ratio();
        let drive_voltage = drive * self.params.drive_gain * supply_ratio;
        let idle_bias = self.params.idle_current * self.params.cathode_resistance;
        let bias_offset = (self.cathode_bias_voltage - idle_bias) * 0.030;

        let (plate_a, positive_current, positive_screen_current) =
            self.solve_plate(self.plate_a_voltage, drive_voltage - bias_offset);
        let (plate_b, negative_current, negative_screen_current) =
            self.solve_plate(self.plate_b_voltage, -drive_voltage - bias_offset);
        let (positive_load_current, negative_load_current) =
            self.reflected_load_current(plate_a, plate_b, drive_voltage);
        let positive_total_plate_current = positive_current + positive_load_current;
        let negative_total_plate_current = negative_current + negative_load_current;
        let total_plate_current = positive_total_plate_current + negative_total_plate_current;
        let total_screen_current = positive_screen_current + negative_screen_current;
        self.update_attack_current(total_plate_current);
        let attack_current = self.attack_current * self.params.attack_current_coupling;
        let total_cathode_current = total_plate_current + total_screen_current + attack_current;

        self.plate_a_voltage = plate_a;
        self.plate_b_voltage = plate_b;
        self.positive_current = positive_total_plate_current;
        self.negative_current = negative_total_plate_current;
        self.positive_load_current = positive_load_current;
        self.negative_load_current = negative_load_current;
        self.positive_screen_current = positive_screen_current;
        self.negative_screen_current = negative_screen_current;
        self.update_cathode_bias(total_cathode_current);
        self.update_screen(total_screen_current + attack_current * 0.35);
        self.update_supply(total_cathode_current, sag);

        let plate_a_signal = self.plate_a_voltage - self.reference_plate_a_voltage;
        let plate_b_signal = self.plate_b_voltage - self.reference_plate_b_voltage;
        (plate_b_signal - plate_a_signal) * self.params.output_scale * self.supply_ratio()
    }

    fn reflected_load_current(
        &self,
        plate_a_voltage: f32,
        plate_b_voltage: f32,
        drive_voltage: f32,
    ) -> (f32, f32) {
        let differential_voltage = (plate_b_voltage - plate_a_voltage).abs();
        let load_current =
            differential_voltage / (self.params.primary_half_resistance * 2.0).max(1.0);
        let stress = (drive_voltage.abs() / 8.0).clamp(0.0, 1.0);
        let coupled_current = load_current * self.params.load_current_coupling * stress;

        if drive_voltage >= 0.0 {
            (coupled_current, coupled_current * 0.18)
        } else {
            (coupled_current * 0.18, coupled_current)
        }
    }

    fn update_attack_current(&mut self, total_plate_current: f32) {
        let idle_plate_current = self.params.idle_current;
        let reference_coefficient = 1.0 - (-1.0 / (self.params.sample_rate * 0.080)).exp();
        self.attack_reference_current +=
            reference_coefficient * (total_plate_current - self.attack_reference_current);

        let excess_current = (total_plate_current - idle_plate_current * 0.68).max(0.0);
        let excess_target = excess_current * excess_current / idle_plate_current.max(1e-6);
        let transient_current =
            (total_plate_current - self.attack_reference_current * 1.03).max(0.0);
        let transient_target = transient_current * 1.65;
        let target = excess_target.max(transient_target);
        let time_constant = if target > self.attack_current {
            0.0035
        } else {
            0.035
        };
        let coefficient = 1.0 - (-1.0 / (self.params.sample_rate * time_constant)).exp();
        self.attack_current += coefficient * (target - self.attack_current);
        self.attack_current = self.attack_current.clamp(0.0, idle_plate_current * 1.4);
    }

    fn solve_plate(&self, previous_plate_voltage: f32, grid_drive: f32) -> (f32, f32, f32) {
        let mut plate_voltage = previous_plate_voltage.clamp(1.0, self.supply_voltage);
        let pentode = self.pentode_point(plate_voltage, grid_drive);
        let residual = (self.supply_voltage - plate_voltage) / self.params.primary_half_resistance
            - pentode.current;
        let derivative = -1.0 / self.params.primary_half_resistance - pentode.d_current_d_plate;
        if derivative.abs() > 1e-12 {
            plate_voltage = (plate_voltage - residual / derivative).clamp(1.0, self.supply_voltage);
        }

        let point = self.pentode_point(plate_voltage, grid_drive);
        (plate_voltage, point.current, point.screen_current)
    }

    fn pentode_point(&self, plate_voltage: f32, grid_drive: f32) -> PentodePoint {
        let plate_to_cathode = (plate_voltage - self.cathode_bias_voltage).max(0.0);
        let screen_to_cathode =
            (self.screen_voltage.min(self.supply_voltage) - self.cathode_bias_voltage).max(0.0);
        let grid_to_cathode = grid_drive - self.cathode_bias_voltage;
        let control = softplus(grid_to_cathode + screen_to_cathode / 42.0, 0.65);
        let saturation = 1.0 - (-plate_to_cathode / 42.0).exp();
        let d_saturation_d_plate = (-plate_to_cathode / 42.0).exp() / 42.0;
        let screen_factor =
            (screen_to_cathode / self.params.screen_voltage.max(1.0)).clamp(0.0, 1.2);
        let shaped = self.params.current_gain * control.powf(1.32) * screen_factor
            / (1.0 + control * self.params.compression);
        let low_plate_screen_pull = (1.0 - plate_to_cathode / screen_to_cathode.max(1.0))
            .clamp(0.0, 1.0)
            .powi(2);
        let screen_current = shaped
            * (0.055 + 0.11 * low_plate_screen_pull)
            * (0.55 + 0.45 * saturation)
            * screen_factor;

        PentodePoint {
            current: (shaped * saturation).clamp(0.0, 0.090),
            screen_current: screen_current.clamp(0.0, 0.018),
            d_current_d_plate: (shaped * d_saturation_d_plate).max(0.0),
        }
    }

    fn update_screen(&mut self, total_screen_current: f32) {
        let target = self
            .params
            .screen_voltage
            .min(self.supply_voltage - total_screen_current * self.params.screen_resistance);
        let coefficient = 1.0
            - (-1.0
                / (self.params.sample_rate
                    * self.params.screen_resistance
                    * self.params.screen_capacitance))
                .exp();
        self.screen_voltage += coefficient * (target - self.screen_voltage);
        self.screen_voltage = self.screen_voltage.clamp(
            self.params.screen_voltage * 0.45,
            self.params.screen_voltage.min(self.supply_voltage),
        );
    }

    fn update_supply(&mut self, total_current: f32, sag: f32) {
        let effective_current = total_current * (0.18 + sag.clamp(0.0, 1.0) * 1.35);
        let target =
            self.params.nominal_supply_voltage - effective_current * self.params.supply_resistance;
        let coefficient = 1.0
            - (-1.0
                / (self.params.sample_rate
                    * self.params.supply_resistance
                    * self.params.supply_capacitance))
                .exp();
        self.supply_voltage += coefficient * (target - self.supply_voltage);
        self.supply_voltage = self.supply_voltage.clamp(
            self.params.nominal_supply_voltage * 0.45,
            self.params.nominal_supply_voltage,
        );
    }

    fn update_cathode_bias(&mut self, total_current: f32) {
        let target = total_current * self.params.cathode_resistance;
        let coefficient = 1.0
            - (-1.0
                / (self.params.sample_rate
                    * self.params.cathode_resistance
                    * self.params.cathode_capacitance))
                .exp();
        self.cathode_bias_voltage += coefficient * (target - self.cathode_bias_voltage);
    }

    fn supply_ratio(&self) -> f32 {
        (self.supply_voltage / self.params.nominal_supply_voltage).clamp(0.45, 1.05)
    }
}

fn smooth_voltage(
    previous: f32,
    target: f32,
    sample_rate: f32,
    resistance: f32,
    capacitance: f32,
) -> f32 {
    let coefficient = 1.0 - (-1.0 / (sample_rate * resistance * capacitance)).exp();
    previous + coefficient * (target - previous)
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

fn softplus(value: f32, scale: f32) -> f32 {
    let normalized = value / scale;
    if normalized > 20.0 {
        value
    } else if normalized < -20.0 {
        0.0
    } else {
        scale * normalized.exp().ln_1p()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn stage() -> PushPullEl84Stage {
        PushPullEl84Stage::new(PushPullEl84Params {
            sample_rate: 48_000.0,
            nominal_supply_voltage: 320.0,
            screen_voltage: 300.0,
            screen_resistance: 1_800.0,
            screen_capacitance: 10e-6,
            primary_half_resistance: 3_200.0,
            supply_resistance: 520.0,
            supply_capacitance: 16e-6,
            cathode_resistance: 130.0,
            cathode_capacitance: 18e-6,
            idle_current: 0.040,
            drive_gain: 18.0,
            current_gain: 0.0048,
            load_current_coupling: 1.35,
            attack_current_coupling: 0.65,
            compression: 0.22,
            output_scale: 0.020,
        })
    }

    fn transformer() -> OutputTransformerStage {
        OutputTransformerStage::new(OutputTransformerParams {
            sample_rate: 48_000.0,
            primary_resistance: 100_000.0,
            primary_inductance: 47.0,
            leakage_cutoff_hz: 13_000.0,
            core_saturation: 1_400.0,
            secondary_saturation_voltage: 1.0,
            output_scale: 1.0,
        })
    }

    fn supply() -> SupplyNetwork {
        SupplyNetwork::new(SupplyNetworkParams {
            sample_rate: 48_000.0,
            rectifier_voltage: 340.0,
            power_nominal_voltage: 320.0,
            phase_inverter_nominal_voltage: 300.0,
            preamp_nominal_voltage: 280.0,
            rectifier_resistance: 420.0,
            phase_inverter_resistance: 10_000.0,
            preamp_resistance: 12_000.0,
            reservoir_capacitance: 32e-6,
            phase_inverter_capacitance: 22e-6,
            preamp_capacitance: 22e-6,
        })
    }

    #[test]
    fn silence_stays_centered_and_finite() {
        let mut stage = stage();
        for _ in 0..2048 {
            let output = stage.process(0.0, 0.5);
            assert!(output.is_finite());
            assert!(output.abs() < 1e-5, "output={output}");
        }
    }

    #[test]
    fn output_is_odd_symmetric_for_small_signal() {
        let mut positive = stage();
        let mut negative = stage();
        for _ in 0..1024 {
            positive.process(0.0, 0.0);
            negative.process(0.0, 0.0);
        }

        let up = positive.process(0.05, 0.0);
        let down = negative.process(-0.05, 0.0);

        assert!((up + down).abs() < up.abs() * 0.12, "up={up}, down={down}");
    }

    #[test]
    fn sustained_drive_drops_supply_voltage() {
        let mut stage = stage();
        let idle_supply = stage.operating_point().supply_voltage;
        for sample_idx in 0..48_000 {
            let input = (std::f32::consts::TAU * 110.0 * sample_idx as f32 / 48_000.0).sin() * 0.7;
            stage.process(input, 1.0);
        }

        let driven_supply = stage.operating_point().supply_voltage;
        assert!(
            driven_supply < idle_supply - 1.0,
            "idle_supply={idle_supply}, driven_supply={driven_supply}"
        );
    }

    #[test]
    fn sustained_drive_drops_screen_voltage() {
        let mut stage = stage();
        for _ in 0..48_000 {
            stage.process(0.0, 0.5);
        }
        let idle = stage.operating_point();

        for sample_idx in 0..24_000 {
            let input = (std::f32::consts::TAU * 110.0 * sample_idx as f32 / 48_000.0).sin() * 1.1;
            stage.process(input, 1.0);
        }
        let driven = stage.operating_point();

        assert!(
            driven.screen_voltage < idle.screen_voltage - 0.5,
            "idle={idle:?}, driven={driven:?}"
        );
    }

    #[test]
    fn attack_raises_screen_current() {
        let mut stage = stage();
        for _ in 0..48_000 {
            stage.process(0.0, 0.5);
        }
        let idle = stage.operating_point();

        for _ in 0..64 {
            stage.process(1.4, 1.0);
        }
        let attacked = stage.operating_point();

        assert!(
            attacked.positive_screen_current + attacked.negative_screen_current
                > idle.positive_screen_current + idle.negative_screen_current,
            "idle={idle:?}, attacked={attacked:?}"
        );
    }

    #[test]
    fn attack_raises_plate_current_demand() {
        let mut quiet = stage();
        let mut attacked = stage();
        for _ in 0..48_000 {
            quiet.process(0.0, 0.5);
            attacked.process(0.0, 0.5);
        }

        let quiet_current = {
            let op = quiet.operating_point();
            op.positive_current + op.negative_current
        };
        let mut peak_current = quiet_current;
        let mut peak_load_current = 0.0;
        for _ in 0..96 {
            attacked.process(1.4, 1.0);
            let op = attacked.operating_point();
            peak_current = f32::max(peak_current, op.positive_current + op.negative_current);
            peak_load_current = f32::max(
                peak_load_current,
                op.positive_load_current + op.negative_load_current,
            );
        }

        assert!(
            peak_current > quiet_current * 1.30,
            "quiet_current={quiet_current}, peak_current={peak_current}"
        );
        assert!(
            peak_load_current > 0.005,
            "peak_load_current={peak_load_current}"
        );
    }

    #[test]
    fn repeated_attacks_move_stored_power_state() {
        let mut stage = stage();
        for _ in 0..48_000 {
            stage.process(0.0, 0.5);
        }
        let idle = stage.operating_point();

        let mut min_supply = idle.supply_voltage;
        let mut min_screen = idle.screen_voltage;
        let mut max_bias = idle.cathode_bias_voltage;
        let mut max_attack_current = 0.0;
        for sample_idx in 0..12_000 {
            let cycle = sample_idx % 1_200;
            let envelope = if cycle < 220 {
                1.0 - cycle as f32 / 440.0
            } else {
                0.18
            };
            let input =
                (std::f32::consts::TAU * 110.0 * sample_idx as f32 / 48_000.0).sin() * envelope;
            stage.process(input, 1.0);
            let op = stage.operating_point();
            min_supply = f32::min(min_supply, op.supply_voltage);
            min_screen = f32::min(min_screen, op.screen_voltage);
            max_bias = f32::max(max_bias, op.cathode_bias_voltage);
            max_attack_current = f32::max(max_attack_current, op.attack_current);
        }

        assert!(
            min_supply < idle.supply_voltage - 3.5,
            "idle={idle:?}, min_supply={min_supply}"
        );
        assert!(
            min_screen < idle.screen_voltage - 1.2,
            "idle={idle:?}, min_screen={min_screen}"
        );
        assert!(
            max_bias > idle.cathode_bias_voltage + 0.60,
            "idle={idle:?}, max_bias={max_bias}"
        );
        assert!(
            max_attack_current > 0.002,
            "max_attack_current={max_attack_current}"
        );
    }

    #[test]
    fn settled_pick_transients_raise_attack_current() {
        let mut stage = stage();
        for _ in 0..48_000 {
            stage.process(0.0, 0.5);
        }

        let mut max_attack_current = 0.0;
        for sample_idx in 0..9_600 {
            let cycle = sample_idx % 1_200;
            let envelope = if cycle < 70 { 1.0 } else { 0.16 };
            let input =
                (std::f32::consts::TAU * 147.0 * sample_idx as f32 / 48_000.0).sin() * envelope;
            stage.process(input, 1.0);
            max_attack_current =
                f32::max(max_attack_current, stage.operating_point().attack_current);
        }

        assert!(
            max_attack_current > 0.0015,
            "max_attack_current={max_attack_current}"
        );
    }

    #[test]
    fn screen_voltage_recovers_after_overload() {
        let mut stage = stage();
        for _ in 0..48_000 {
            stage.process(0.0, 0.5);
        }
        let idle_screen = stage.operating_point().screen_voltage;

        for sample_idx in 0..24_000 {
            let input = (std::f32::consts::TAU * 110.0 * sample_idx as f32 / 48_000.0).sin() * 1.2;
            stage.process(input, 1.0);
        }
        let sagged_screen = stage.operating_point().screen_voltage;

        for _ in 0..96_000 {
            stage.process(0.0, 0.5);
        }
        let recovered_screen = stage.operating_point().screen_voltage;

        assert!(
            recovered_screen > sagged_screen + 0.5,
            "idle_screen={idle_screen}, sagged_screen={sagged_screen}, recovered_screen={recovered_screen}"
        );
        assert!(
            (idle_screen - recovered_screen).abs() < (idle_screen - sagged_screen).abs(),
            "idle_screen={idle_screen}, sagged_screen={sagged_screen}, recovered_screen={recovered_screen}"
        );
    }

    #[test]
    fn cathode_bias_recovers_after_overload() {
        let mut stage = stage();
        for _ in 0..48_000 {
            stage.process(0.0, 0.5);
        }
        let idle_bias = stage.operating_point().cathode_bias_voltage;

        for sample_idx in 0..12_000 {
            let input = (std::f32::consts::TAU * 110.0 * sample_idx as f32 / 48_000.0).sin() * 1.4;
            stage.process(input, 0.5);
        }
        let overloaded_bias = stage.operating_point().cathode_bias_voltage;

        for _ in 0..48_000 {
            stage.process(0.0, 0.5);
        }
        let recovered_bias = stage.operating_point().cathode_bias_voltage;

        assert!(
            (recovered_bias - idle_bias).abs() < (overloaded_bias - idle_bias).abs(),
            "idle_bias={idle_bias}, overloaded_bias={overloaded_bias}, recovered_bias={recovered_bias}"
        );
    }

    #[test]
    fn processing_cost_stays_below_realtime_budget() {
        let mut stage = stage();
        let sample_count = 48_000;
        let start = Instant::now();
        let mut sum = 0.0;
        for sample_idx in 0..sample_count {
            let input = (std::f32::consts::TAU * 110.0 * sample_idx as f32 / 48_000.0).sin() * 0.7;
            sum += stage.process(input, 0.7);
        }
        let elapsed = start.elapsed();

        assert!(sum.is_finite());
        assert!(
            elapsed < Duration::from_millis(150),
            "elapsed={elapsed:?} for {sample_count} samples"
        );
    }

    #[test]
    fn transformer_blocks_dc() {
        let mut transformer = transformer();
        let mut sum = 0.0;
        for sample_idx in 0..48_000 {
            let output = transformer.process(0.5);
            if sample_idx >= 47_000 {
                sum += output.abs();
            }
        }

        assert!(sum / 1_000.0 < 0.01, "settled_dc={}", sum / 1_000.0);
    }

    #[test]
    fn transformer_rolls_off_leakage_highs() {
        let mut low = transformer();
        let mut high = transformer();
        let low_rms = transformer_sine_rms(&mut low, 1_000.0, 0.2);
        let high_rms = transformer_sine_rms(&mut high, 18_000.0, 0.2);

        assert!(
            low_rms > high_rms * 1.15,
            "low_rms={low_rms}, high_rms={high_rms}"
        );
    }

    #[test]
    fn transformer_core_flux_compresses_sustained_low_end() {
        let mut light = transformer();
        let mut heavy = transformer();
        let light_rms = transformer_sine_rms(&mut light, 80.0, 0.1);
        let heavy_rms = transformer_sine_rms(&mut heavy, 80.0, 1.0);
        let linear_ratio = heavy_rms / light_rms;

        assert!(
            linear_ratio < 9.4,
            "light_rms={light_rms}, heavy_rms={heavy_rms}, linear_ratio={linear_ratio}"
        );
    }

    #[test]
    fn transformer_reset_clears_flux_history() {
        let mut transformer = transformer();
        for sample_idx in 0..12_000 {
            let input = (std::f32::consts::TAU * 80.0 * sample_idx as f32 / 48_000.0).sin();
            transformer.process(input);
        }
        assert!(transformer.operating_point().core_flux.abs() > 0.0);
        transformer.reset();
        assert_eq!(transformer.operating_point().core_flux, 0.0);
    }

    #[test]
    fn supply_network_orders_rails_from_power_to_preamp() {
        let mut supply = supply();
        for _ in 0..48_000 {
            supply.process(0.003, 0.002, 0.080, 0.6);
        }
        let operating_point = supply.operating_point();

        assert!(operating_point.power_voltage > operating_point.phase_inverter_voltage);
        assert!(operating_point.phase_inverter_voltage > operating_point.preamp_voltage);
    }

    #[test]
    fn supply_network_sags_under_power_current() {
        let mut quiet = supply();
        let mut loud = supply();
        for _ in 0..48_000 {
            quiet.process(0.002, 0.001, 0.020, 1.0);
            loud.process(0.002, 0.001, 0.120, 1.0);
        }

        assert!(
            loud.operating_point().power_voltage < quiet.operating_point().power_voltage - 10.0,
            "quiet={:?}, loud={:?}",
            quiet.operating_point(),
            loud.operating_point()
        );
    }

    #[test]
    fn supply_network_recovers_after_overload() {
        let mut supply = supply();
        for _ in 0..48_000 {
            supply.process(0.003, 0.002, 0.140, 1.0);
        }
        let sagged = supply.operating_point().power_voltage;
        for _ in 0..96_000 {
            supply.process(0.001, 0.001, 0.020, 0.5);
        }
        let recovered = supply.operating_point().power_voltage;

        assert!(
            recovered > sagged + 10.0,
            "sagged={sagged}, recovered={recovered}"
        );
    }

    fn transformer_sine_rms(
        transformer: &mut OutputTransformerStage,
        frequency: f32,
        amplitude: f32,
    ) -> f32 {
        let mut sum = 0.0;
        let mut count = 0;
        for sample_idx in 0..48_000 {
            let input = (std::f32::consts::TAU * frequency * sample_idx as f32 / 48_000.0).sin()
                * amplitude;
            let output = transformer.process(input);
            if sample_idx >= 24_000 {
                sum += output * output;
                count += 1;
            }
        }
        (sum / count as f32).sqrt()
    }
}
