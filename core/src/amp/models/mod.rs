mod ac30;
mod dumble;
mod jcm800;

use super::AmpControls;
use ac30::Ac30;
use dumble::Dumble;
use jcm800::Jcm800;

pub(in crate::amp) enum AmpCore {
    Ac30(Ac30),
    Dumble(Dumble),
    Jcm800(Jcm800),
}

impl AmpCore {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self::Ac30(Ac30::new(sample_rate))
    }

    pub(super) fn new_with_model(sample_rate: f32, model: &str) -> Self {
        match model {
            "dumble" => Self::Dumble(Dumble::new(sample_rate)),
            "jcm800" | "jcm-800" | "marshall" => Self::Jcm800(Jcm800::new(sample_rate)),
            _ => Self::new(sample_rate),
        }
    }

    pub(super) fn reset(&mut self) {
        match self {
            Self::Ac30(model) => model.reset(),
            Self::Dumble(model) => model.reset(),
            Self::Jcm800(model) => model.reset(),
        }
    }

    #[inline]
    pub(super) fn process(&mut self, input: f32, controls: AmpControls) -> f32 {
        match self {
            Self::Ac30(model) => model.process(input, controls),
            Self::Dumble(model) => model.process(input, controls),
            Self::Jcm800(model) => model.process(input, controls),
        }
    }
}

pub(super) trait AmpModel {
    fn reset(&mut self);
    fn process(&mut self, input: f32, controls: AmpControls) -> f32;
}
