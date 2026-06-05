#[derive(Clone, Copy)]
pub struct PushPullEl84Params {
    pub sample_rate: f32,
    pub nominal_supply_voltage: f32,
    pub screen_voltage: f32,
    pub primary_half_resistance: f32,
    pub supply_resistance: f32,
    pub supply_capacitance: f32,
    pub cathode_resistance: f32,
    pub cathode_capacitance: f32,
    pub idle_current: f32,
    pub drive_gain: f32,
    pub current_gain: f32,
    pub compression: f32,
    pub output_scale: f32,
}

pub struct PushPullEl84Stage {
    params: PushPullEl84Params,
    supply_voltage: f32,
    cathode_bias_voltage: f32,
    plate_a_voltage: f32,
    plate_b_voltage: f32,
    reference_plate_a_voltage: f32,
    reference_plate_b_voltage: f32,
    positive_current: f32,
    negative_current: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PushPullEl84OperatingPoint {
    pub supply_voltage: f32,
    pub plate_a_voltage: f32,
    pub plate_b_voltage: f32,
    pub cathode_bias_voltage: f32,
    pub positive_current: f32,
    pub negative_current: f32,
}

#[derive(Clone, Copy)]
struct PentodePoint {
    current: f32,
    d_current_d_plate: f32,
}

impl PushPullEl84Stage {
    pub fn new(params: PushPullEl84Params) -> Self {
        let idle_cathode = params.idle_current * params.cathode_resistance;
        let idle_plate_drop = params.idle_current * 0.5 * params.primary_half_resistance;
        let idle_plate = params.nominal_supply_voltage - idle_plate_drop;
        let mut stage = Self {
            params,
            supply_voltage: params.nominal_supply_voltage,
            cathode_bias_voltage: idle_cathode,
            plate_a_voltage: idle_plate,
            plate_b_voltage: idle_plate,
            reference_plate_a_voltage: idle_plate,
            reference_plate_b_voltage: idle_plate,
            positive_current: params.idle_current * 0.5,
            negative_current: params.idle_current * 0.5,
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
            plate_a_voltage: self.plate_a_voltage,
            plate_b_voltage: self.plate_b_voltage,
            cathode_bias_voltage: self.cathode_bias_voltage,
            positive_current: self.positive_current,
            negative_current: self.negative_current,
        }
    }

    pub fn process(&mut self, drive: f32, sag: f32) -> f32 {
        let supply_ratio = self.supply_ratio();
        let drive_voltage = drive * self.params.drive_gain * supply_ratio;
        let idle_bias = self.params.idle_current * self.params.cathode_resistance;
        let bias_offset = (self.cathode_bias_voltage - idle_bias) * 0.030;

        let (plate_a, positive_current) =
            self.solve_plate(self.plate_a_voltage, drive_voltage - bias_offset);
        let (plate_b, negative_current) =
            self.solve_plate(self.plate_b_voltage, -drive_voltage - bias_offset);
        let total_current = positive_current + negative_current;

        self.plate_a_voltage = plate_a;
        self.plate_b_voltage = plate_b;
        self.positive_current = positive_current;
        self.negative_current = negative_current;
        self.update_cathode_bias(total_current);
        self.update_supply(total_current, sag);

        let plate_a_signal = self.plate_a_voltage - self.reference_plate_a_voltage;
        let plate_b_signal = self.plate_b_voltage - self.reference_plate_b_voltage;
        (plate_b_signal - plate_a_signal) * self.params.output_scale * self.supply_ratio()
    }

    fn solve_plate(&self, previous_plate_voltage: f32, grid_drive: f32) -> (f32, f32) {
        let mut plate_voltage = previous_plate_voltage.clamp(1.0, self.supply_voltage);
        let pentode = self.pentode_point(plate_voltage, grid_drive);
        let residual = (self.supply_voltage - plate_voltage) / self.params.primary_half_resistance
            - pentode.current;
        let derivative = -1.0 / self.params.primary_half_resistance - pentode.d_current_d_plate;
        if derivative.abs() > 1e-12 {
            plate_voltage = (plate_voltage - residual / derivative).clamp(1.0, self.supply_voltage);
        }

        let current = self.pentode_point(plate_voltage, grid_drive).current;
        (plate_voltage, current)
    }

    fn pentode_point(&self, plate_voltage: f32, grid_drive: f32) -> PentodePoint {
        let plate_to_cathode = (plate_voltage - self.cathode_bias_voltage).max(0.0);
        let screen_to_cathode = (self.params.screen_voltage.min(self.supply_voltage)
            - self.cathode_bias_voltage)
            .max(0.0);
        let grid_to_cathode = grid_drive - self.cathode_bias_voltage;
        let control = softplus(grid_to_cathode + screen_to_cathode / 42.0, 0.65);
        let saturation = 1.0 - (-plate_to_cathode / 42.0).exp();
        let d_saturation_d_plate = (-plate_to_cathode / 42.0).exp() / 42.0;
        let screen_factor =
            (screen_to_cathode / self.params.screen_voltage.max(1.0)).clamp(0.0, 1.2);
        let shaped = self.params.current_gain * control.powf(1.32) * screen_factor
            / (1.0 + control * self.params.compression);

        PentodePoint {
            current: (shaped * saturation).clamp(0.0, 0.090),
            d_current_d_plate: (shaped * d_saturation_d_plate).max(0.0),
        }
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
            primary_half_resistance: 3_200.0,
            supply_resistance: 360.0,
            supply_capacitance: 32e-6,
            cathode_resistance: 130.0,
            cathode_capacitance: 50e-6,
            idle_current: 0.040,
            drive_gain: 18.0,
            current_gain: 0.0048,
            compression: 0.22,
            output_scale: 0.020,
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
            elapsed < Duration::from_millis(100),
            "elapsed={elapsed:?} for {sample_count} samples"
        );
    }
}
