const NEWTON_ITERATIONS: usize = 18;
const NEWTON_TOLERANCE: f32 = 1e-8;

#[derive(Clone, Copy)]
pub struct TriodeParams {
    pub mu: f32,
    pub ex: f32,
    pub kg1: f32,
    pub kp: f32,
    pub kvb: f32,
}

impl TriodeParams {
    pub const ECC83: Self = Self {
        mu: 100.0,
        ex: 1.4,
        kg1: 1060.0,
        kp: 600.0,
        kvb: 300.0,
    };
}

#[derive(Clone, Copy)]
pub struct CommonCathodeParams {
    pub sample_rate: f32,
    pub grid_leak_resistance: f32,
    pub input_coupling_capacitance: f32,
    pub plate_resistance: f32,
    pub cathode_resistance: f32,
    pub cathode_bypass_capacitance: Option<f32>,
    pub supply_resistance: f32,
    pub supply_capacitance: f32,
    pub nominal_supply_voltage: f32,
    pub input_gain: f32,
    pub output_scale: f32,
    pub triode: TriodeParams,
}

pub struct CommonCathodeStage {
    params: CommonCathodeParams,
    supply_voltage: f32,
    input_coupling: CouplingCapacitor,
    cathode_bypass: Option<GroundedCapacitor>,
    last_grid_voltage: f32,
    last_plate_voltage: f32,
    last_cathode_voltage: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct CommonCathodeOperatingPoint {
    pub grid_voltage: f32,
    pub plate_voltage: f32,
    pub cathode_voltage: f32,
    pub supply_voltage: f32,
}

#[derive(Clone, Copy)]
pub struct CathodeFollowerParams {
    pub sample_rate: f32,
    pub grid_leak_resistance: f32,
    pub input_coupling_capacitance: f32,
    pub cathode_resistance: f32,
    pub nominal_supply_voltage: f32,
    pub input_gain: f32,
    pub output_scale: f32,
    pub triode: TriodeParams,
}

pub struct CathodeFollowerStage {
    params: CathodeFollowerParams,
    input_coupling: CouplingCapacitor,
    last_grid_voltage: f32,
    last_cathode_voltage: f32,
    reference_cathode_voltage: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct CathodeFollowerOperatingPoint {
    pub grid_voltage: f32,
    pub cathode_voltage: f32,
    pub supply_voltage: f32,
}

impl CommonCathodeStage {
    pub fn new(params: CommonCathodeParams) -> Self {
        let quiescent_plate = params.nominal_supply_voltage * 0.62;
        let input_coupling =
            CouplingCapacitor::new(params.input_coupling_capacitance, params.sample_rate);
        let cathode_bypass = params
            .cathode_bypass_capacitance
            .map(|capacitance| GroundedCapacitor::new(capacitance, params.sample_rate));
        Self {
            params,
            supply_voltage: params.nominal_supply_voltage,
            input_coupling,
            cathode_bypass,
            last_grid_voltage: 0.0,
            last_plate_voltage: quiescent_plate,
            last_cathode_voltage: 1.5,
        }
    }

    pub fn reset(&mut self) {
        self.supply_voltage = self.params.nominal_supply_voltage;
        self.input_coupling.reset();
        if let Some(capacitor) = &mut self.cathode_bypass {
            capacitor.reset();
        }
        self.last_grid_voltage = 0.0;
        self.last_plate_voltage = self.params.nominal_supply_voltage * 0.62;
        self.last_cathode_voltage = 1.5;
    }

    pub fn supply_voltage(&self) -> f32 {
        self.supply_voltage
    }

    pub fn operating_point(&self) -> CommonCathodeOperatingPoint {
        CommonCathodeOperatingPoint {
            grid_voltage: self.last_grid_voltage,
            plate_voltage: self.last_plate_voltage,
            cathode_voltage: self.last_cathode_voltage,
            supply_voltage: self.supply_voltage,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let input_voltage = input * self.params.input_gain;
        let operating_point = self.solve_operating_point(input_voltage);
        let plate_current = triode_current(
            self.params.triode,
            operating_point.plate_voltage,
            operating_point.grid_voltage,
            operating_point.cathode_voltage,
        );

        self.input_coupling
            .update(operating_point.grid_voltage, input_voltage);
        self.update_cathode_bypass(operating_point.cathode_voltage);
        self.update_supply(plate_current);
        self.last_grid_voltage = operating_point.grid_voltage;
        self.last_plate_voltage = operating_point.plate_voltage;
        self.last_cathode_voltage = operating_point.cathode_voltage;

        let centered_plate = operating_point.plate_voltage - self.supply_voltage * 0.62;
        -centered_plate * self.params.output_scale
    }

    fn solve_operating_point(&self, input_voltage: f32) -> CommonCathodeOperatingPoint {
        let mut plate_voltage = self
            .last_plate_voltage
            .clamp(1.0, self.supply_voltage.max(1.0));
        let mut cathode_voltage = self
            .last_cathode_voltage
            .clamp(0.0, self.supply_voltage.max(1.0));
        let mut grid_voltage = self.last_grid_voltage.clamp(-50.0, 10.0);

        for _ in 0..NEWTON_ITERATIONS {
            let residuals =
                self.residuals(plate_voltage, cathode_voltage, grid_voltage, input_voltage);

            if residuals.iter().copied().map(f32::abs).fold(0.0, f32::max) < NEWTON_TOLERANCE {
                break;
            }

            let jacobian =
                self.jacobian(plate_voltage, cathode_voltage, grid_voltage, input_voltage);
            let Some(delta) = solve_3x3(jacobian, [-residuals[0], -residuals[1], -residuals[2]])
            else {
                break;
            };

            plate_voltage = (plate_voltage + delta[0]).clamp(1.0, self.supply_voltage.max(1.0));
            cathode_voltage = (cathode_voltage + delta[1]).clamp(0.0, self.supply_voltage);
            grid_voltage = (grid_voltage + delta[2]).clamp(-50.0, 10.0);
        }

        CommonCathodeOperatingPoint {
            grid_voltage,
            plate_voltage,
            cathode_voltage,
            supply_voltage: self.supply_voltage,
        }
    }

    fn residuals(
        &self,
        plate_voltage: f32,
        cathode_voltage: f32,
        grid_voltage: f32,
        input_voltage: f32,
    ) -> [f32; 3] {
        let plate_current = triode_current(
            self.params.triode,
            plate_voltage,
            grid_voltage,
            cathode_voltage,
        );
        let grid_current = grid_current(grid_voltage, cathode_voltage);
        let cathode_resistor_current = cathode_voltage / self.params.cathode_resistance;
        let cathode_bypass_current = self
            .cathode_bypass
            .as_ref()
            .map_or(0.0, |capacitor| capacitor.current_at(cathode_voltage));
        let coupling_current = self.input_coupling.current_at(grid_voltage, input_voltage);
        let grid_leak_current = grid_voltage / self.params.grid_leak_resistance;

        [
            (self.supply_voltage - plate_voltage) / self.params.plate_resistance - plate_current,
            plate_current + grid_current - cathode_resistor_current - cathode_bypass_current,
            coupling_current + grid_leak_current + grid_current,
        ]
    }

    fn jacobian(
        &self,
        plate_voltage: f32,
        cathode_voltage: f32,
        grid_voltage: f32,
        input_voltage: f32,
    ) -> [[f32; 3]; 3] {
        let variables = [plate_voltage, cathode_voltage, grid_voltage];
        let steps = [0.05, 0.01, 0.01];
        let mut jacobian = [[0.0; 3]; 3];

        for column in 0..3 {
            let mut plus = variables;
            let mut minus = variables;
            plus[column] += steps[column];
            minus[column] -= steps[column];
            let plus_residuals = self.residuals(plus[0], plus[1], plus[2], input_voltage);
            let minus_residuals = self.residuals(minus[0], minus[1], minus[2], input_voltage);
            for row in 0..3 {
                jacobian[row][column] =
                    (plus_residuals[row] - minus_residuals[row]) / (2.0 * steps[column]);
            }
        }

        jacobian
    }

    fn update_supply(&mut self, plate_current: f32) {
        let target =
            self.params.nominal_supply_voltage - plate_current * self.params.supply_resistance;
        let coefficient = 1.0
            - (-1.0
                / (self.params.sample_rate
                    * self.params.supply_resistance
                    * self.params.supply_capacitance))
                .exp();
        self.supply_voltage += coefficient * (target - self.supply_voltage);
    }

    fn update_cathode_bypass(&mut self, cathode_voltage: f32) {
        if let Some(capacitor) = &mut self.cathode_bypass {
            capacitor.update(cathode_voltage);
        }
    }
}

impl CathodeFollowerStage {
    pub fn new(params: CathodeFollowerParams) -> Self {
        let input_coupling =
            CouplingCapacitor::new(params.input_coupling_capacitance, params.sample_rate);
        let mut stage = Self {
            params,
            input_coupling,
            last_grid_voltage: 0.0,
            last_cathode_voltage: 1.5,
            reference_cathode_voltage: 1.5,
        };
        let operating_point = stage.solve_operating_point(0.0);
        stage.last_grid_voltage = operating_point.grid_voltage;
        stage.last_cathode_voltage = operating_point.cathode_voltage;
        stage.reference_cathode_voltage = operating_point.cathode_voltage;
        stage
    }

    pub fn reset(&mut self) {
        self.input_coupling.reset();
        let operating_point = self.solve_operating_point(0.0);
        self.last_grid_voltage = operating_point.grid_voltage;
        self.last_cathode_voltage = operating_point.cathode_voltage;
        self.reference_cathode_voltage = operating_point.cathode_voltage;
    }

    pub fn operating_point(&self) -> CathodeFollowerOperatingPoint {
        CathodeFollowerOperatingPoint {
            grid_voltage: self.last_grid_voltage,
            cathode_voltage: self.last_cathode_voltage,
            supply_voltage: self.params.nominal_supply_voltage,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let input_voltage = input * self.params.input_gain;
        let operating_point = self.solve_operating_point(input_voltage);
        self.input_coupling
            .update(operating_point.grid_voltage, input_voltage);
        self.last_grid_voltage = operating_point.grid_voltage;
        self.last_cathode_voltage = operating_point.cathode_voltage;

        (operating_point.cathode_voltage - self.reference_cathode_voltage)
            * self.params.output_scale
    }

    fn solve_operating_point(&self, input_voltage: f32) -> CathodeFollowerOperatingPoint {
        let mut cathode_voltage = self
            .last_cathode_voltage
            .clamp(0.0, self.params.nominal_supply_voltage);
        let mut grid_voltage = self.last_grid_voltage.clamp(-50.0, 50.0);

        for _ in 0..NEWTON_ITERATIONS {
            let residuals = self.residuals(cathode_voltage, grid_voltage, input_voltage);
            if residuals.iter().copied().map(f32::abs).fold(0.0, f32::max) < NEWTON_TOLERANCE {
                break;
            }

            let jacobian = self.jacobian(cathode_voltage, grid_voltage, input_voltage);
            let determinant = jacobian[0][0] * jacobian[1][1] - jacobian[0][1] * jacobian[1][0];
            if determinant.abs() < 1e-12 {
                break;
            }

            let delta_cathode =
                (-residuals[0] * jacobian[1][1] + jacobian[0][1] * residuals[1]) / determinant;
            let delta_grid =
                (jacobian[1][0] * residuals[0] - jacobian[0][0] * residuals[1]) / determinant;

            cathode_voltage =
                (cathode_voltage + delta_cathode).clamp(0.0, self.params.nominal_supply_voltage);
            grid_voltage = (grid_voltage + delta_grid).clamp(-50.0, 50.0);
        }

        CathodeFollowerOperatingPoint {
            grid_voltage,
            cathode_voltage,
            supply_voltage: self.params.nominal_supply_voltage,
        }
    }

    fn residuals(&self, cathode_voltage: f32, grid_voltage: f32, input_voltage: f32) -> [f32; 2] {
        let plate_current = triode_current(
            self.params.triode,
            self.params.nominal_supply_voltage,
            grid_voltage,
            cathode_voltage,
        );
        let grid_current = grid_current(grid_voltage, cathode_voltage);
        let cathode_resistor_current = cathode_voltage / self.params.cathode_resistance;
        let coupling_current = self.input_coupling.current_at(grid_voltage, input_voltage);
        let grid_leak_current = grid_voltage / self.params.grid_leak_resistance;

        [
            plate_current + grid_current - cathode_resistor_current,
            coupling_current + grid_leak_current + grid_current,
        ]
    }

    fn jacobian(
        &self,
        cathode_voltage: f32,
        grid_voltage: f32,
        input_voltage: f32,
    ) -> [[f32; 2]; 2] {
        let variables = [cathode_voltage, grid_voltage];
        let steps = [0.01, 0.01];
        let mut jacobian = [[0.0; 2]; 2];

        for column in 0..2 {
            let mut plus = variables;
            let mut minus = variables;
            plus[column] += steps[column];
            minus[column] -= steps[column];
            let plus_residuals = self.residuals(plus[0], plus[1], input_voltage);
            let minus_residuals = self.residuals(minus[0], minus[1], input_voltage);
            for row in 0..2 {
                jacobian[row][column] =
                    (plus_residuals[row] - minus_residuals[row]) / (2.0 * steps[column]);
            }
        }

        jacobian
    }
}

struct CouplingCapacitor {
    conductance: f32,
    previous_voltage: f32,
    previous_current: f32,
}

impl CouplingCapacitor {
    fn new(capacitance: f32, sample_rate: f32) -> Self {
        Self {
            conductance: 2.0 * capacitance * sample_rate,
            previous_voltage: 0.0,
            previous_current: 0.0,
        }
    }

    fn current_at(&self, grid_voltage: f32, input_voltage: f32) -> f32 {
        let capacitor_voltage = grid_voltage - input_voltage;
        let history_current = -self.conductance * self.previous_voltage - self.previous_current;
        self.conductance * capacitor_voltage + history_current
    }

    fn update(&mut self, grid_voltage: f32, input_voltage: f32) {
        let capacitor_voltage = grid_voltage - input_voltage;
        self.previous_current =
            self.conductance * (capacitor_voltage - self.previous_voltage) - self.previous_current;
        self.previous_voltage = capacitor_voltage;
    }

    fn reset(&mut self) {
        self.previous_voltage = 0.0;
        self.previous_current = 0.0;
    }
}

struct GroundedCapacitor {
    conductance: f32,
    previous_voltage: f32,
    previous_current: f32,
}

impl GroundedCapacitor {
    fn new(capacitance: f32, sample_rate: f32) -> Self {
        Self {
            conductance: 2.0 * capacitance * sample_rate,
            previous_voltage: 0.0,
            previous_current: 0.0,
        }
    }

    fn current_at(&self, voltage: f32) -> f32 {
        let history_current = -self.conductance * self.previous_voltage - self.previous_current;
        self.conductance * voltage + history_current
    }

    fn update(&mut self, voltage: f32) {
        self.previous_current =
            self.conductance * (voltage - self.previous_voltage) - self.previous_current;
        self.previous_voltage = voltage;
    }

    fn reset(&mut self) {
        self.previous_voltage = 0.0;
        self.previous_current = 0.0;
    }
}

fn triode_current(
    params: TriodeParams,
    plate_voltage: f32,
    grid_voltage: f32,
    cathode_voltage: f32,
) -> f32 {
    let plate_to_cathode = (plate_voltage - cathode_voltage).max(0.0);
    let grid_to_cathode = grid_voltage - cathode_voltage;

    let shaping = (params.kp * (1.0 / params.mu + grid_to_cathode / plate_to_cathode.max(1.0)))
        .exp()
        .ln_1p()
        / params.kp;
    let knee = (plate_to_cathode / params.kvb).sqrt();
    let conduction = (plate_to_cathode / params.kg1) * shaping.max(0.0).powf(params.ex) * knee;
    conduction.clamp(0.0, 0.040)
}

fn grid_current(grid_voltage: f32, cathode_voltage: f32) -> f32 {
    let grid_to_cathode = grid_voltage - cathode_voltage;
    let overdrive = softplus(grid_to_cathode, 0.04);
    ((overdrive * overdrive) / 50_000.0).clamp(0.0, 0.005)
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

fn solve_3x3(mut matrix: [[f32; 3]; 3], mut rhs: [f32; 3]) -> Option<[f32; 3]> {
    for pivot in 0..3 {
        let mut pivot_row = pivot;
        let mut pivot_abs = matrix[pivot][pivot].abs();
        for (row, values) in matrix.iter().enumerate().skip(pivot + 1) {
            let candidate_abs = values[pivot].abs();
            if candidate_abs > pivot_abs {
                pivot_abs = candidate_abs;
                pivot_row = row;
            }
        }

        if pivot_abs < 1e-12 {
            return None;
        }

        if pivot_row != pivot {
            matrix.swap(pivot, pivot_row);
            rhs.swap(pivot, pivot_row);
        }

        for row in (pivot + 1)..3 {
            let factor = matrix[row][pivot] / matrix[pivot][pivot];
            for column in pivot..3 {
                matrix[row][column] -= factor * matrix[pivot][column];
            }
            rhs[row] -= factor * rhs[pivot];
        }
    }

    let mut solution = [0.0; 3];
    for row in (0..3).rev() {
        let mut sum = rhs[row];
        for (column, value) in solution.iter().enumerate().skip(row + 1) {
            sum -= matrix[row][column] * value;
        }
        solution[row] = sum / matrix[row][row];
    }

    Some(solution)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage() -> CommonCathodeStage {
        CommonCathodeStage::new(CommonCathodeParams {
            sample_rate: 48_000.0,
            grid_leak_resistance: 1_000_000.0,
            input_coupling_capacitance: 22e-9,
            plate_resistance: 100_000.0,
            cathode_resistance: 1_500.0,
            cathode_bypass_capacitance: Some(25e-6),
            supply_resistance: 10_000.0,
            supply_capacitance: 22e-6,
            nominal_supply_voltage: 280.0,
            input_gain: 1.8,
            output_scale: 0.018,
            triode: TriodeParams::ECC83,
        })
    }

    fn follower() -> CathodeFollowerStage {
        CathodeFollowerStage::new(CathodeFollowerParams {
            sample_rate: 48_000.0,
            grid_leak_resistance: 1_000_000.0,
            input_coupling_capacitance: 47e-9,
            cathode_resistance: 100_000.0,
            nominal_supply_voltage: 280.0,
            input_gain: 1.0,
            output_scale: 1.0,
            triode: TriodeParams::ECC83,
        })
    }

    #[test]
    fn silence_converges_to_finite_operating_point() {
        let mut stage = stage();
        for _ in 0..1024 {
            assert!(stage.process(0.0).is_finite());
        }
        assert!(stage.supply_voltage().is_finite());
    }

    #[test]
    fn idle_operating_point_tracks_spice_fixture() {
        let mut stage = stage();
        settle_idle(&mut stage);
        let operating_point = stage.operating_point();

        assert_close(operating_point.plate_voltage, 250.54, 2.0, "plate");
        assert_close(operating_point.cathode_voltage, 0.40, 0.05, "cathode");
        assert_close(operating_point.supply_voltage, 277.32, 0.5, "supply");
        assert_close(operating_point.grid_voltage, 0.0, 0.005, "grid");
    }

    #[test]
    fn small_signal_gain_tracks_spice_fixture() {
        let mut stage = stage();
        settle_idle(&mut stage);
        let input_voltage_rms = 0.020 / std::f32::consts::SQRT_2;
        let input_amplitude = 0.020 / stage.params.input_gain;
        let response = node_rms(&mut stage, 1_000.0, input_amplitude);
        let plate_voltage_rms = response.plate;
        let gain = plate_voltage_rms / input_voltage_rms;

        assert_close(response.grid, 0.01412, 0.001, "grid rms");
        assert_close(response.cathode, 0.000013, 0.0005, "cathode rms");
        assert_close(gain, 14.88, 1.5, "small-signal gain");
    }

    #[test]
    fn sustained_drive_modulates_supply_voltage() {
        let mut stage = stage();
        settle_idle(&mut stage);
        let idle_supply = stage.supply_voltage();

        for sample_idx in 0..24_000 {
            let input = (std::f32::consts::TAU * 220.0 * sample_idx as f32 / 48_000.0).sin() * 0.8;
            stage.process(input);
        }

        assert!(
            (stage.supply_voltage() - idle_supply).abs() > 0.05,
            "idle={}, driven={}",
            idle_supply,
            stage.supply_voltage()
        );
    }

    #[test]
    fn positive_grid_overload_recovers_through_grid_leak() {
        let mut quiet = stage();
        let mut loud = stage();

        for sample_idx in 0..12_000 {
            let phase = std::f32::consts::TAU * 110.0 * sample_idx as f32 / 48_000.0;
            quiet.process(phase.sin() * 0.1);
            loud.process(phase.sin() * 0.8);
        }

        let quiet_grid = quiet.operating_point().grid_voltage;
        let overloaded_grid = loud.operating_point().grid_voltage;

        for _ in 0..48_000 {
            loud.process(0.0);
        }

        let recovered_grid = loud.operating_point().grid_voltage;

        assert!(
            (overloaded_grid - quiet_grid).abs() > 0.05,
            "quiet_grid={quiet_grid}, overloaded_grid={overloaded_grid}"
        );
        assert!(
            recovered_grid.abs() < overloaded_grid.abs(),
            "overloaded_grid={overloaded_grid}, recovered_grid={recovered_grid}"
        );
    }

    #[test]
    fn input_coupling_blocks_dc() {
        let mut dc_stage = stage();
        let mut reference = stage();
        let mut dc_settled = 0.0;
        let mut reference_settled = 0.0;

        for sample_idx in 0..96_000 {
            let dc_output = dc_stage.process(0.25);
            let reference_output = reference.process(0.0);
            if sample_idx >= 95_000 {
                dc_settled += dc_output;
                reference_settled += reference_output;
            }
        }

        assert!(((dc_settled - reference_settled) / 1_000.0).abs() < 0.01);
    }

    #[test]
    fn cathode_bypass_changes_midband_gain() {
        let mut bypassed = stage();
        let mut unbypassed = CommonCathodeStage::new(CommonCathodeParams {
            cathode_bypass_capacitance: None,
            ..stage().params
        });

        settle_idle(&mut bypassed);
        settle_idle(&mut unbypassed);

        let bypassed_level = node_rms(&mut bypassed, 1_000.0, 0.001).plate;
        let unbypassed_level = node_rms(&mut unbypassed, 1_000.0, 0.001).plate;
        let ratio = bypassed_level / unbypassed_level;

        assert!(
            ratio > 1.08,
            "bypassed={bypassed_level}, unbypassed={unbypassed_level}, ratio={ratio}"
        );
    }

    #[test]
    fn cathode_follower_biases_to_finite_operating_point() {
        let mut follower = follower();
        for _ in 0..1024 {
            assert!(follower.process(0.0).is_finite());
        }

        let operating_point = follower.operating_point();
        assert!(operating_point.cathode_voltage > 0.2);
        assert!(operating_point.cathode_voltage < operating_point.supply_voltage);
        assert!(operating_point.grid_voltage.abs() < 0.01);
    }

    #[test]
    fn cathode_follower_tracks_small_signal_below_unity() {
        let mut follower = follower();
        settle_follower_idle(&mut follower);
        let response = follower_rms(&mut follower, 1_000.0, 0.020);
        let input_rms = 0.020 / std::f32::consts::SQRT_2;
        let gain = response / input_rms;

        assert!(
            (0.65..0.99).contains(&gain),
            "response={response}, input_rms={input_rms}, gain={gain}"
        );
    }

    #[test]
    fn cathode_follower_recovers_from_grid_current_blocking() {
        let mut follower = follower();
        for sample_idx in 0..12_000 {
            let input = (std::f32::consts::TAU * 110.0 * sample_idx as f32 / 48_000.0).sin() * 4.0;
            follower.process(input);
        }
        let overloaded_grid = follower.operating_point().grid_voltage;

        for _ in 0..48_000 {
            follower.process(0.0);
        }
        let recovered_grid = follower.operating_point().grid_voltage;

        assert!(
            recovered_grid.abs() < overloaded_grid.abs(),
            "overloaded_grid={overloaded_grid}, recovered_grid={recovered_grid}"
        );
    }

    struct NodeRms {
        grid: f32,
        plate: f32,
        cathode: f32,
    }

    fn node_rms(stage: &mut CommonCathodeStage, frequency: f32, amplitude: f32) -> NodeRms {
        let mut grid_samples = Vec::new();
        let mut plate_samples = Vec::new();
        let mut cathode_samples = Vec::new();
        for sample_idx in 0..24_000 {
            let input = (std::f32::consts::TAU * frequency * sample_idx as f32 / 48_000.0).sin()
                * amplitude;
            stage.process(input);
            if sample_idx >= 12_000 {
                let operating_point = stage.operating_point();
                grid_samples.push(operating_point.grid_voltage);
                plate_samples.push(operating_point.plate_voltage);
                cathode_samples.push(operating_point.cathode_voltage);
            }
        }
        NodeRms {
            grid: rms(&grid_samples),
            plate: rms(&plate_samples),
            cathode: rms(&cathode_samples),
        }
    }

    fn settle_idle(stage: &mut CommonCathodeStage) {
        for _ in 0..96_000 {
            stage.process(0.0);
        }
    }

    fn settle_follower_idle(stage: &mut CathodeFollowerStage) {
        for _ in 0..96_000 {
            stage.process(0.0);
        }
    }

    fn follower_rms(stage: &mut CathodeFollowerStage, frequency: f32, amplitude: f32) -> f32 {
        let mut samples = Vec::new();
        for sample_idx in 0..24_000 {
            let input = (std::f32::consts::TAU * frequency * sample_idx as f32 / 48_000.0).sin()
                * amplitude;
            let output = stage.process(input);
            if sample_idx >= 12_000 {
                samples.push(output);
            }
        }
        rms(&samples)
    }

    fn rms(samples: &[f32]) -> f32 {
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        (samples
            .iter()
            .map(|sample| {
                let centered = sample - mean;
                centered * centered
            })
            .sum::<f32>()
            / samples.len() as f32)
            .sqrt()
    }

    fn assert_close(actual: f32, expected: f32, tolerance: f32, label: &str) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{label}: actual={actual}, expected={expected}, tolerance={tolerance}"
        );
    }
}
