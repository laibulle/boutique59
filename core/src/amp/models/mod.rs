mod dumbler;
mod nox30;
mod sheriff800;

use super::{AmpControls, NeuralCellMode, Nox30OperatingPoint};
use dumbler::Dumbler;
use nox30::Nox30;
use sheriff800::Sheriff800;
use std::path::PathBuf;

pub(super) fn configure_nox30_first_stage_neural(
    descriptor_path: Option<PathBuf>,
    mode: NeuralCellMode,
) {
    nox30::configure_first_stage_neural(descriptor_path, mode);
}

pub(super) fn configure_nox30_first_stage_graybox(
    config_path: Option<PathBuf>,
    mode: NeuralCellMode,
) {
    nox30::configure_first_stage_graybox(config_path, mode);
}

pub(in crate::amp) enum AmpCore {
    Dumbler(Dumbler),
    Sheriff800(Sheriff800),
    Nox30(Nox30),
}

impl AmpCore {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self::Nox30(Nox30::new(sample_rate))
    }

    pub(super) fn new_with_model(sample_rate: f32, model: &str) -> Self {
        match model {
            "dumbler" => Self::Dumbler(Dumbler::new(sample_rate)),
            "sheriff800" | "sheriff-800" => Self::Sheriff800(Sheriff800::new(sample_rate)),
            "nox30" => Self::Nox30(Nox30::new(sample_rate)),
            _ => Self::new(sample_rate),
        }
    }

    pub(super) fn reset(&mut self) {
        match self {
            Self::Dumbler(model) => model.reset(),
            Self::Sheriff800(model) => model.reset(),
            Self::Nox30(model) => model.reset(),
        }
    }

    pub(super) fn nox30_operating_point(&self) -> Option<Nox30OperatingPoint> {
        match self {
            Self::Nox30(model) => Some(model.operating_point()),
            _ => None,
        }
    }

    #[inline]
    pub(super) fn process(&mut self, input: f32, controls: AmpControls) -> f32 {
        match self {
            Self::Dumbler(model) => model.process(input, controls),
            Self::Sheriff800(model) => model.process(input, controls),
            Self::Nox30(model) => model.process(input, controls),
        }
    }

    #[inline]
    pub(super) fn process_with_fx_loop(
        &mut self,
        input: f32,
        controls: AmpControls,
        mut process_fx: impl FnMut(f32) -> f32,
    ) -> f32 {
        match self {
            Self::Nox30(model) => {
                let preamp = model.process_preamp(input, controls);
                let return_voltage = process_fx(preamp.send_voltage);
                model.process_power_amp(return_voltage, preamp, controls)
            }
            _ => process_fx(self.process(input, controls)),
        }
    }
}

pub(super) trait AmpModel {
    fn reset(&mut self);
    fn process(&mut self, input: f32, controls: AmpControls) -> f32;
}
