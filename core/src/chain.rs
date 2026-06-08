use crate::amp::{AmpControls, VoxAmp};
use crate::pedal::{
    Brigade, BrigadeControls, Celeste, CelesteControls, ConnectionState, Dartford,
    DartfordControls, ElectricalSignal, GodessOne, GodessOneControls, Jetstream, JetstreamControls,
    Load, Lumen, LumenControls, Minotaur, MinotaurControls, Monarch, MonarchControls, Muffin,
    MuffinControls, Muon, MuonControls, Springfield, SpringfieldControls, Tron, TronControls,
    AMP_INPUT_IMPEDANCE_OHMS, GUITAR_SOURCE_IMPEDANCE_OHMS,
};

const DEFAULT_CABLE_CAPACITANCE_FARADS: f32 = 470e-12;
const AMP_OUTPUT_SOURCE_IMPEDANCE_OHMS: f32 = 10_000.0;
const FX_SEND_SOURCE_IMPEDANCE_OHMS: f32 = 1_000.0;
const FX_RETURN_INPUT_IMPEDANCE_OHMS: f32 = 1_000_000.0;

#[derive(Clone, Debug)]
pub struct SignalChainConfig {
    pub amp_model: String,
    pub pre_amp: Vec<DeviceSlotConfig>,
    pub fx_loop: Vec<DeviceSlotConfig>,
    pub post_amp: Vec<DeviceSlotConfig>,
    pub cable_capacitance_farads: f32,
}

impl SignalChainConfig {
    pub fn amp_only(model: &str) -> Self {
        Self {
            amp_model: model.to_string(),
            pre_amp: Vec::new(),
            fx_loop: Vec::new(),
            post_amp: Vec::new(),
            cable_capacitance_farads: DEFAULT_CABLE_CAPACITANCE_FARADS,
        }
    }

    pub fn with_pre_amp_device(mut self, device: DeviceConfig) -> Self {
        self.pre_amp.push(DeviceSlotConfig::active(device));
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceSlotConfig {
    pub device: DeviceConfig,
    pub bypassed: bool,
}

impl DeviceSlotConfig {
    pub fn active(device: DeviceConfig) -> Self {
        Self {
            device,
            bypassed: false,
        }
    }

    pub fn bypassed(device: DeviceConfig) -> Self {
        Self {
            device,
            bypassed: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceConfig {
    Lumen,
    Muon,
    Muffin,
    Minotaur,
    Monarch,
    GodessOne,
    Dartford,
    Tron,
    Jetstream,
    Celeste,
    Brigade,
    Springfield,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlKind {
    Pot,
    Slider,
    Switch,
    Footswitch,
}

#[derive(Clone, Copy, Debug)]
pub struct ControlDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ControlKind,
    pub min: f32,
    pub max: f32,
    pub display_scale: f32,
    pub step: f32,
    pub large_step: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceVisualDescriptor {
    pub width: usize,
    pub color: &'static str,
    pub bypass_color: &'static str,
    pub active_color: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceModelDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub visual: DeviceVisualDescriptor,
    pub controls: &'static [ControlDescriptor],
}

#[derive(Clone, Copy, Debug)]
pub struct AmpModelDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub visual: DeviceVisualDescriptor,
}

const STANDARD_PEDAL_VISUAL: DeviceVisualDescriptor = DeviceVisualDescriptor {
    width: 14,
    color: "steel",
    bypass_color: "gray",
    active_color: "green",
};

const STANDARD_AMP_VISUAL: DeviceVisualDescriptor = DeviceVisualDescriptor {
    width: 18,
    color: "black",
    bypass_color: "gray",
    active_color: "amber",
};

const MUFFIN_CONTROLS: &[ControlDescriptor] = &[
    unit_pot("sustain", "sustain"),
    unit_pot("tone", "tone"),
    unit_pot("level", "level"),
];
const MINOTAUR_CONTROLS: &[ControlDescriptor] = &[
    unit_pot("gain", "gain"),
    unit_pot("treble", "treble"),
    unit_pot("output", "output"),
];
const MONARCH_CONTROLS: &[ControlDescriptor] = &[
    unit_pot("gain", "gain"),
    unit_pot("tone", "tone"),
    unit_pot("output", "output"),
];
const GODESS_ONE_CONTROLS: &[ControlDescriptor] = &[
    unit_pot("distortion", "dist"),
    unit_pot("tone", "tone"),
    unit_pot("level", "level"),
    ControlDescriptor {
        id: "mode",
        label: "mode",
        kind: ControlKind::Switch,
        min: 0.0,
        max: 1.0,
        display_scale: 1.0,
        step: 1.0,
        large_step: 1.0,
    },
];
const DARTFORD_CONTROLS: &[ControlDescriptor] = &[
    ControlDescriptor {
        id: "rate_hz",
        label: "rate Hz",
        kind: ControlKind::Slider,
        min: 0.05,
        max: 20.0,
        display_scale: 1.0,
        step: 0.1,
        large_step: 1.0,
    },
    unit_pot("depth", "depth"),
    ControlDescriptor {
        id: "level",
        label: "level",
        kind: ControlKind::Pot,
        min: 0.0,
        max: 2.0,
        display_scale: 10.0,
        step: 0.01,
        large_step: 0.1,
    },
    ControlDescriptor {
        id: "wave",
        label: "wave",
        kind: ControlKind::Switch,
        min: 0.0,
        max: 2.0,
        display_scale: 1.0,
        step: 1.0,
        large_step: 1.0,
    },
];
const SPRINGFIELD_CONTROLS: &[ControlDescriptor] = &[
    unit_pot("dwell", "dwell"),
    unit_pot("tone", "tone"),
    unit_pot("mix", "mix"),
];
const TRON_CONTROLS: &[ControlDescriptor] = &[
    ControlDescriptor {
        id: "rate_hz",
        label: "rate Hz",
        kind: ControlKind::Slider,
        min: 0.03,
        max: 12.0,
        display_scale: 1.0,
        step: 0.05,
        large_step: 0.5,
    },
    unit_pot("depth", "depth"),
    unit_pot("feedback", "fdbk"),
    unit_pot("mix", "mix"),
];
const JETSTREAM_CONTROLS: &[ControlDescriptor] = &[
    unit_pot("manual", "manual"),
    ControlDescriptor {
        id: "rate_hz",
        label: "rate Hz",
        kind: ControlKind::Slider,
        min: 0.02,
        max: 8.0,
        display_scale: 1.0,
        step: 0.05,
        large_step: 0.5,
    },
    unit_pot("depth", "depth"),
    unit_pot("feedback", "fdbk"),
    unit_pot("mix", "mix"),
];
const CELESTE_CONTROLS: &[ControlDescriptor] = &[
    ControlDescriptor {
        id: "rate_hz",
        label: "rate Hz",
        kind: ControlKind::Slider,
        min: 0.05,
        max: 6.0,
        display_scale: 1.0,
        step: 0.05,
        large_step: 0.5,
    },
    unit_pot("depth", "depth"),
    unit_pot("tone", "tone"),
    unit_pot("mix", "mix"),
];
const BRIGADE_CONTROLS: &[ControlDescriptor] = &[
    ControlDescriptor {
        id: "time_ms",
        label: "time ms",
        kind: ControlKind::Slider,
        min: 60.0,
        max: 700.0,
        display_scale: 1.0,
        step: 5.0,
        large_step: 25.0,
    },
    unit_pot("repeats", "repeat"),
    unit_pot("tone", "tone"),
    unit_pot("mix", "mix"),
];
const LUMEN_CONTROLS: &[ControlDescriptor] = &[
    unit_pot("peak_reduction", "reduce"),
    unit_pot("gain", "gain"),
    unit_pot("emphasis", "emphasis"),
    unit_pot("mix", "mix"),
];
const MUON_CONTROLS: &[ControlDescriptor] = &[
    unit_pot("sensitivity", "sens"),
    unit_pot("range", "range"),
    unit_pot("resonance", "reso"),
    unit_pot("mix", "mix"),
];

const fn unit_pot(id: &'static str, label: &'static str) -> ControlDescriptor {
    ControlDescriptor {
        id,
        label,
        kind: ControlKind::Pot,
        min: 0.0,
        max: 1.0,
        display_scale: 10.0,
        step: 0.01,
        large_step: 0.1,
    }
}

impl DeviceConfig {
    pub fn model_descriptor(self) -> DeviceModelDescriptor {
        match self {
            Self::Lumen => DeviceModelDescriptor {
                id: "lumen",
                label: "Lumen",
                category: "compressor",
                visual: DeviceVisualDescriptor {
                    color: "silver",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: LUMEN_CONTROLS,
            },
            Self::Muon => DeviceModelDescriptor {
                id: "muon",
                label: "Muon",
                category: "filter",
                visual: DeviceVisualDescriptor {
                    color: "seafoam",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: MUON_CONTROLS,
            },
            Self::Muffin => DeviceModelDescriptor {
                id: "muffin",
                label: "Muffin",
                category: "fuzz",
                visual: DeviceVisualDescriptor {
                    color: "olive",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: MUFFIN_CONTROLS,
            },
            Self::Minotaur => DeviceModelDescriptor {
                id: "minotaur",
                label: "Minotaur",
                category: "overdrive",
                visual: DeviceVisualDescriptor {
                    color: "gold",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: MINOTAUR_CONTROLS,
            },
            Self::Monarch => DeviceModelDescriptor {
                id: "monarch",
                label: "Monarch",
                category: "overdrive",
                visual: DeviceVisualDescriptor {
                    color: "royal-purple",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: MONARCH_CONTROLS,
            },
            Self::GodessOne => DeviceModelDescriptor {
                id: "godess-one",
                label: "Godess One",
                category: "distortion",
                visual: DeviceVisualDescriptor {
                    color: "orange",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: GODESS_ONE_CONTROLS,
            },
            Self::Dartford => DeviceModelDescriptor {
                id: "dartford",
                label: "Dartford",
                category: "modulation",
                visual: DeviceVisualDescriptor {
                    color: "teal",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: DARTFORD_CONTROLS,
            },
            Self::Tron => DeviceModelDescriptor {
                id: "tron",
                label: "Tron",
                category: "modulation",
                visual: DeviceVisualDescriptor {
                    color: "lamp-orange",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: TRON_CONTROLS,
            },
            Self::Jetstream => DeviceModelDescriptor {
                id: "jetstream",
                label: "Jetstream",
                category: "modulation",
                visual: DeviceVisualDescriptor {
                    color: "sky-blue",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: JETSTREAM_CONTROLS,
            },
            Self::Celeste => DeviceModelDescriptor {
                id: "celeste",
                label: "Celeste",
                category: "modulation",
                visual: DeviceVisualDescriptor {
                    color: "cobalt-blue",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: CELESTE_CONTROLS,
            },
            Self::Brigade => DeviceModelDescriptor {
                id: "brigade",
                label: "Brigade",
                category: "delay",
                visual: DeviceVisualDescriptor {
                    color: "violet",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: BRIGADE_CONTROLS,
            },
            Self::Springfield => DeviceModelDescriptor {
                id: "springfield",
                label: "Springfield",
                category: "reverb",
                visual: DeviceVisualDescriptor {
                    color: "surf-green",
                    ..STANDARD_PEDAL_VISUAL
                },
                controls: SPRINGFIELD_CONTROLS,
            },
        }
    }
}

pub fn amp_model_descriptor(model: &str) -> AmpModelDescriptor {
    match model {
        "nox30" => AmpModelDescriptor {
            id: "nox30",
            label: "Nox30",
            visual: DeviceVisualDescriptor {
                color: "copper",
                ..STANDARD_AMP_VISUAL
            },
        },
        "sheriff800" | "sheriff-800" => AmpModelDescriptor {
            id: "sheriff800",
            label: "Sheriff800",
            visual: DeviceVisualDescriptor {
                color: "black-gold",
                ..STANDARD_AMP_VISUAL
            },
        },
        "dumbler" => AmpModelDescriptor {
            id: "dumbler",
            label: "Dumbler",
            visual: DeviceVisualDescriptor {
                color: "tan",
                ..STANDARD_AMP_VISUAL
            },
        },
        _ => AmpModelDescriptor {
            id: "unknown",
            label: "Amp",
            visual: STANDARD_AMP_VISUAL,
        },
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SignalChainControls<'a> {
    pub amp: AmpControls,
    pub devices: &'a [DeviceSlotControls],
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceSlotControls {
    pub bypassed: bool,
    pub controls: DeviceControls,
}

impl DeviceSlotControls {
    pub fn active(controls: DeviceControls) -> Self {
        Self {
            bypassed: false,
            controls,
        }
    }

    pub fn bypassed(controls: DeviceControls) -> Self {
        Self {
            bypassed: true,
            controls,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DeviceControls {
    Default,
    Lumen(LumenControls),
    Muon(MuonControls),
    Muffin(MuffinControls),
    Minotaur(MinotaurControls),
    Monarch(MonarchControls),
    GodessOne(GodessOneControls),
    Dartford(DartfordControls),
    Tron(TronControls),
    Jetstream(JetstreamControls),
    Celeste(CelesteControls),
    Brigade(BrigadeControls),
    Springfield(SpringfieldControls),
}

pub struct SignalChain {
    amp: VoxAmp,
    pre_amp: Vec<DeviceSlot>,
    fx_loop: Vec<DeviceSlot>,
    post_amp: Vec<DeviceSlot>,
    amp_input_connection: ConnectionState,
    fx_return_connection: ConnectionState,
}

impl SignalChain {
    pub fn new(sample_rate: f32, config: SignalChainConfig) -> Self {
        let cable_capacitance = config.cable_capacitance_farads.max(0.0);
        Self {
            amp: VoxAmp::with_model(sample_rate, &config.amp_model),
            pre_amp: config
                .pre_amp
                .into_iter()
                .map(|slot| DeviceSlot::new(sample_rate, cable_capacitance, slot))
                .collect(),
            fx_loop: config
                .fx_loop
                .into_iter()
                .map(|slot| DeviceSlot::new(sample_rate, cable_capacitance, slot))
                .collect(),
            post_amp: config
                .post_amp
                .into_iter()
                .map(|slot| DeviceSlot::new(sample_rate, cable_capacitance, slot))
                .collect(),
            amp_input_connection: ConnectionState::new(sample_rate, cable_capacitance),
            fx_return_connection: ConnectionState::new(sample_rate, cable_capacitance),
        }
    }

    pub fn reset(&mut self) {
        for slot in self
            .pre_amp
            .iter_mut()
            .chain(self.fx_loop.iter_mut())
            .chain(self.post_amp.iter_mut())
        {
            slot.reset();
        }
        self.amp_input_connection.reset();
        self.fx_return_connection.reset();
        self.amp.reset();
    }

    pub fn process(&mut self, input: f32, controls: SignalChainControls<'_>) -> f32 {
        self.process_with_amp_enabled(input, controls, true)
    }

    pub fn process_with_amp_enabled(
        &mut self,
        input: f32,
        controls: SignalChainControls<'_>,
        amp_enabled: bool,
    ) -> f32 {
        let pre_amp_start = 0;
        let fx_loop_start = self.pre_amp.len();
        let post_amp_start = fx_loop_start + self.fx_loop.len();

        let has_active_pre_amp = has_active_slots(&self.pre_amp, controls, pre_amp_start);
        let has_active_fx_loop = has_active_slots(&self.fx_loop, controls, fx_loop_start);
        let has_active_post_amp = has_active_slots(&self.post_amp, controls, post_amp_start);

        if !has_active_pre_amp && !has_active_fx_loop && !has_active_post_amp {
            return if amp_enabled {
                self.amp.process(input, controls.amp)
            } else {
                input
            };
        }

        let mut signal = ElectricalSignal::new(input, GUITAR_SOURCE_IMPEDANCE_OHMS);
        signal = process_slots(&mut self.pre_amp, signal, controls, pre_amp_start);

        if !amp_enabled {
            signal = process_slots(&mut self.post_amp, signal, controls, post_amp_start);
            return signal.voltage;
        }

        let amp_input = if has_active_pre_amp {
            self.amp_input_connection
                .drive_load(signal, Load::new(AMP_INPUT_IMPEDANCE_OHMS))
        } else {
            input
        };

        let amp_output = if has_active_fx_loop {
            let fx_loop = &mut self.fx_loop;
            let fx_return_connection = &mut self.fx_return_connection;
            self.amp
                .process_with_fx_loop(amp_input, controls.amp, |send_voltage| {
                    let mut send_signal =
                        ElectricalSignal::new(send_voltage, FX_SEND_SOURCE_IMPEDANCE_OHMS);
                    send_signal = process_slots(fx_loop, send_signal, controls, fx_loop_start);
                    fx_return_connection
                        .drive_load(send_signal, Load::new(FX_RETURN_INPUT_IMPEDANCE_OHMS))
                })
        } else {
            self.amp.process(amp_input, controls.amp)
        };
        let mut output_signal = ElectricalSignal::new(amp_output, AMP_OUTPUT_SOURCE_IMPEDANCE_OHMS);
        output_signal = process_slots(&mut self.post_amp, output_signal, controls, post_amp_start);
        output_signal.voltage
    }

    pub fn nox30_operating_point(&self) -> Option<crate::amp::Nox30OperatingPoint> {
        self.amp.nox30_operating_point()
    }
}

fn process_slots(
    slots: &mut [DeviceSlot],
    mut signal: ElectricalSignal,
    controls: SignalChainControls<'_>,
    slot_start_index: usize,
) -> ElectricalSignal {
    for (section_index, slot) in slots.iter_mut().enumerate() {
        let slot_index = slot_start_index + section_index;
        signal = slot.process(signal, controls, slot_index);
    }
    signal
}

fn has_active_slots(
    slots: &[DeviceSlot],
    controls: SignalChainControls<'_>,
    slot_start_index: usize,
) -> bool {
    slots
        .iter()
        .enumerate()
        .any(|(section_index, slot)| !slot.is_bypassed(controls, slot_start_index + section_index))
}

struct DeviceSlot {
    bypassed: bool,
    input_connection: ConnectionState,
    processor: DeviceProcessor,
}

impl DeviceSlot {
    fn new(sample_rate: f32, cable_capacitance_farads: f32, config: DeviceSlotConfig) -> Self {
        Self {
            bypassed: config.bypassed,
            input_connection: ConnectionState::new(sample_rate, cable_capacitance_farads),
            processor: DeviceProcessor::new(sample_rate, config.device),
        }
    }

    fn reset(&mut self) {
        self.input_connection.reset();
        self.processor.reset();
    }

    fn process(
        &mut self,
        source: ElectricalSignal,
        controls: SignalChainControls<'_>,
        slot_index: usize,
    ) -> ElectricalSignal {
        if self.is_bypassed(controls, slot_index) {
            return source;
        }

        let device_controls = self.device_controls(controls, slot_index);
        let loaded_input = self
            .input_connection
            .drive_load(source, Load::new(self.processor.input_impedance_ohms()));
        self.processor
            .process_loaded_voltage(loaded_input, device_controls)
    }

    fn is_bypassed(&self, controls: SignalChainControls<'_>, slot_index: usize) -> bool {
        controls
            .devices
            .get(slot_index)
            .map(|slot_controls| slot_controls.bypassed)
            .unwrap_or(self.bypassed)
    }

    fn device_controls(
        &self,
        controls: SignalChainControls<'_>,
        slot_index: usize,
    ) -> DeviceControls {
        controls
            .devices
            .get(slot_index)
            .map(|slot_controls| slot_controls.controls)
            .unwrap_or(DeviceControls::Default)
    }
}

enum DeviceProcessor {
    Lumen(Lumen),
    Muon(Muon),
    Muffin(Muffin),
    Minotaur(Minotaur),
    Monarch(Monarch),
    GodessOne(GodessOne),
    Dartford(Dartford),
    Tron(Tron),
    Jetstream(Jetstream),
    Celeste(Celeste),
    Brigade(Brigade),
    Springfield(Springfield),
}

impl DeviceProcessor {
    fn new(sample_rate: f32, config: DeviceConfig) -> Self {
        match config {
            DeviceConfig::Lumen => Self::Lumen(Lumen::new(sample_rate)),
            DeviceConfig::Muon => Self::Muon(Muon::new(sample_rate)),
            DeviceConfig::Muffin => Self::Muffin(Muffin::new(sample_rate)),
            DeviceConfig::Minotaur => Self::Minotaur(Minotaur::new(sample_rate)),
            DeviceConfig::Monarch => Self::Monarch(Monarch::new(sample_rate)),
            DeviceConfig::GodessOne => Self::GodessOne(GodessOne::new(sample_rate)),
            DeviceConfig::Dartford => Self::Dartford(Dartford::new(sample_rate)),
            DeviceConfig::Tron => Self::Tron(Tron::new(sample_rate)),
            DeviceConfig::Jetstream => Self::Jetstream(Jetstream::new(sample_rate)),
            DeviceConfig::Celeste => Self::Celeste(Celeste::new(sample_rate)),
            DeviceConfig::Brigade => Self::Brigade(Brigade::new(sample_rate)),
            DeviceConfig::Springfield => Self::Springfield(Springfield::new(sample_rate)),
        }
    }

    fn reset(&mut self) {
        match self {
            Self::Lumen(pedal) => pedal.reset(),
            Self::Muon(pedal) => pedal.reset(),
            Self::Muffin(pedal) => pedal.reset(),
            Self::Minotaur(pedal) => pedal.reset(),
            Self::Monarch(pedal) => pedal.reset(),
            Self::GodessOne(pedal) => pedal.reset(),
            Self::Dartford(pedal) => pedal.reset(),
            Self::Tron(pedal) => pedal.reset(),
            Self::Jetstream(pedal) => pedal.reset(),
            Self::Celeste(pedal) => pedal.reset(),
            Self::Brigade(pedal) => pedal.reset(),
            Self::Springfield(pedal) => pedal.reset(),
        }
    }

    fn input_impedance_ohms(&self) -> f32 {
        match self {
            Self::Lumen(_) => Lumen::INPUT_IMPEDANCE_OHMS,
            Self::Muon(_) => Muon::INPUT_IMPEDANCE_OHMS,
            Self::Muffin(_) => Muffin::INPUT_IMPEDANCE_OHMS,
            Self::Minotaur(_) => Minotaur::INPUT_IMPEDANCE_OHMS,
            Self::Monarch(_) => Monarch::INPUT_IMPEDANCE_OHMS,
            Self::GodessOne(_) => GodessOne::INPUT_IMPEDANCE_OHMS,
            Self::Dartford(_) => Dartford::INPUT_IMPEDANCE_OHMS,
            Self::Tron(_) => Tron::INPUT_IMPEDANCE_OHMS,
            Self::Jetstream(_) => Jetstream::INPUT_IMPEDANCE_OHMS,
            Self::Celeste(_) => Celeste::INPUT_IMPEDANCE_OHMS,
            Self::Brigade(_) => Brigade::INPUT_IMPEDANCE_OHMS,
            Self::Springfield(_) => Springfield::INPUT_IMPEDANCE_OHMS,
        }
    }

    fn process_loaded_voltage(
        &mut self,
        input_voltage: f32,
        controls: DeviceControls,
    ) -> ElectricalSignal {
        match self {
            Self::Lumen(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Lumen(controls) => controls,
                    DeviceControls::Default => LumenControls::default(),
                    DeviceControls::Muon(_) => LumenControls::default(),
                    DeviceControls::Muffin(_) => LumenControls::default(),
                    DeviceControls::Minotaur(_) => LumenControls::default(),
                    DeviceControls::Monarch(_) => LumenControls::default(),
                    DeviceControls::GodessOne(_) => LumenControls::default(),
                    DeviceControls::Dartford(_) => LumenControls::default(),
                    DeviceControls::Tron(_) => LumenControls::default(),
                    DeviceControls::Jetstream(_) => LumenControls::default(),
                    DeviceControls::Celeste(_) => LumenControls::default(),
                    DeviceControls::Brigade(_) => LumenControls::default(),
                    DeviceControls::Springfield(_) => LumenControls::default(),
                },
            ),
            Self::Muon(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Muon(controls) => controls,
                    DeviceControls::Default => MuonControls::default(),
                    DeviceControls::Lumen(_) => MuonControls::default(),
                    DeviceControls::Muffin(_) => MuonControls::default(),
                    DeviceControls::Minotaur(_) => MuonControls::default(),
                    DeviceControls::Monarch(_) => MuonControls::default(),
                    DeviceControls::GodessOne(_) => MuonControls::default(),
                    DeviceControls::Dartford(_) => MuonControls::default(),
                    DeviceControls::Tron(_) => MuonControls::default(),
                    DeviceControls::Jetstream(_) => MuonControls::default(),
                    DeviceControls::Celeste(_) => MuonControls::default(),
                    DeviceControls::Brigade(_) => MuonControls::default(),
                    DeviceControls::Springfield(_) => MuonControls::default(),
                },
            ),
            Self::Muffin(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Muffin(controls) => controls,
                    DeviceControls::Default => MuffinControls::default(),
                    DeviceControls::Lumen(_) => MuffinControls::default(),
                    DeviceControls::Muon(_) => MuffinControls::default(),
                    DeviceControls::Minotaur(_) => MuffinControls::default(),
                    DeviceControls::Monarch(_) => MuffinControls::default(),
                    DeviceControls::GodessOne(_) => MuffinControls::default(),
                    DeviceControls::Dartford(_) => MuffinControls::default(),
                    DeviceControls::Tron(_) => MuffinControls::default(),
                    DeviceControls::Jetstream(_) => MuffinControls::default(),
                    DeviceControls::Celeste(_) => MuffinControls::default(),
                    DeviceControls::Brigade(_) => MuffinControls::default(),
                    DeviceControls::Springfield(_) => MuffinControls::default(),
                },
            ),
            Self::Minotaur(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Minotaur(controls) => controls,
                    DeviceControls::Default => MinotaurControls::default(),
                    DeviceControls::Lumen(_) => MinotaurControls::default(),
                    DeviceControls::Muon(_) => MinotaurControls::default(),
                    DeviceControls::Muffin(_) => MinotaurControls::default(),
                    DeviceControls::Monarch(_) => MinotaurControls::default(),
                    DeviceControls::GodessOne(_) => MinotaurControls::default(),
                    DeviceControls::Dartford(_) => MinotaurControls::default(),
                    DeviceControls::Tron(_) => MinotaurControls::default(),
                    DeviceControls::Jetstream(_) => MinotaurControls::default(),
                    DeviceControls::Celeste(_) => MinotaurControls::default(),
                    DeviceControls::Brigade(_) => MinotaurControls::default(),
                    DeviceControls::Springfield(_) => MinotaurControls::default(),
                },
            ),
            Self::Monarch(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Monarch(controls) => controls,
                    DeviceControls::Default => MonarchControls::default(),
                    DeviceControls::Lumen(_) => MonarchControls::default(),
                    DeviceControls::Muon(_) => MonarchControls::default(),
                    DeviceControls::Muffin(_) => MonarchControls::default(),
                    DeviceControls::Minotaur(_) => MonarchControls::default(),
                    DeviceControls::GodessOne(_) => MonarchControls::default(),
                    DeviceControls::Dartford(_) => MonarchControls::default(),
                    DeviceControls::Tron(_) => MonarchControls::default(),
                    DeviceControls::Jetstream(_) => MonarchControls::default(),
                    DeviceControls::Celeste(_) => MonarchControls::default(),
                    DeviceControls::Brigade(_) => MonarchControls::default(),
                    DeviceControls::Springfield(_) => MonarchControls::default(),
                },
            ),
            Self::GodessOne(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::GodessOne(controls) => controls,
                    DeviceControls::Default => GodessOneControls::default(),
                    DeviceControls::Lumen(_) => GodessOneControls::default(),
                    DeviceControls::Muon(_) => GodessOneControls::default(),
                    DeviceControls::Muffin(_) => GodessOneControls::default(),
                    DeviceControls::Minotaur(_) => GodessOneControls::default(),
                    DeviceControls::Monarch(_) => GodessOneControls::default(),
                    DeviceControls::Dartford(_) => GodessOneControls::default(),
                    DeviceControls::Tron(_) => GodessOneControls::default(),
                    DeviceControls::Jetstream(_) => GodessOneControls::default(),
                    DeviceControls::Celeste(_) => GodessOneControls::default(),
                    DeviceControls::Brigade(_) => GodessOneControls::default(),
                    DeviceControls::Springfield(_) => GodessOneControls::default(),
                },
            ),
            Self::Dartford(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Dartford(controls) => controls,
                    DeviceControls::Default => DartfordControls::default(),
                    DeviceControls::Lumen(_) => DartfordControls::default(),
                    DeviceControls::Muon(_) => DartfordControls::default(),
                    DeviceControls::Muffin(_) => DartfordControls::default(),
                    DeviceControls::Minotaur(_) => DartfordControls::default(),
                    DeviceControls::Monarch(_) => DartfordControls::default(),
                    DeviceControls::GodessOne(_) => DartfordControls::default(),
                    DeviceControls::Tron(_) => DartfordControls::default(),
                    DeviceControls::Jetstream(_) => DartfordControls::default(),
                    DeviceControls::Celeste(_) => DartfordControls::default(),
                    DeviceControls::Brigade(_) => DartfordControls::default(),
                    DeviceControls::Springfield(_) => DartfordControls::default(),
                },
            ),
            Self::Tron(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Tron(controls) => controls,
                    DeviceControls::Default => TronControls::default(),
                    DeviceControls::Lumen(_) => TronControls::default(),
                    DeviceControls::Muon(_) => TronControls::default(),
                    DeviceControls::Muffin(_) => TronControls::default(),
                    DeviceControls::Minotaur(_) => TronControls::default(),
                    DeviceControls::Monarch(_) => TronControls::default(),
                    DeviceControls::GodessOne(_) => TronControls::default(),
                    DeviceControls::Dartford(_) => TronControls::default(),
                    DeviceControls::Jetstream(_) => TronControls::default(),
                    DeviceControls::Celeste(_) => TronControls::default(),
                    DeviceControls::Brigade(_) => TronControls::default(),
                    DeviceControls::Springfield(_) => TronControls::default(),
                },
            ),
            Self::Jetstream(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Jetstream(controls) => controls,
                    DeviceControls::Default => JetstreamControls::default(),
                    DeviceControls::Lumen(_) => JetstreamControls::default(),
                    DeviceControls::Muon(_) => JetstreamControls::default(),
                    DeviceControls::Muffin(_) => JetstreamControls::default(),
                    DeviceControls::Minotaur(_) => JetstreamControls::default(),
                    DeviceControls::Monarch(_) => JetstreamControls::default(),
                    DeviceControls::GodessOne(_) => JetstreamControls::default(),
                    DeviceControls::Dartford(_) => JetstreamControls::default(),
                    DeviceControls::Tron(_) => JetstreamControls::default(),
                    DeviceControls::Celeste(_) => JetstreamControls::default(),
                    DeviceControls::Brigade(_) => JetstreamControls::default(),
                    DeviceControls::Springfield(_) => JetstreamControls::default(),
                },
            ),
            Self::Celeste(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Celeste(controls) => controls,
                    DeviceControls::Default => CelesteControls::default(),
                    DeviceControls::Lumen(_) => CelesteControls::default(),
                    DeviceControls::Muon(_) => CelesteControls::default(),
                    DeviceControls::Muffin(_) => CelesteControls::default(),
                    DeviceControls::Minotaur(_) => CelesteControls::default(),
                    DeviceControls::Monarch(_) => CelesteControls::default(),
                    DeviceControls::GodessOne(_) => CelesteControls::default(),
                    DeviceControls::Dartford(_) => CelesteControls::default(),
                    DeviceControls::Tron(_) => CelesteControls::default(),
                    DeviceControls::Jetstream(_) => CelesteControls::default(),
                    DeviceControls::Brigade(_) => CelesteControls::default(),
                    DeviceControls::Springfield(_) => CelesteControls::default(),
                },
            ),
            Self::Brigade(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Brigade(controls) => controls,
                    DeviceControls::Default => BrigadeControls::default(),
                    DeviceControls::Lumen(_) => BrigadeControls::default(),
                    DeviceControls::Muon(_) => BrigadeControls::default(),
                    DeviceControls::Muffin(_) => BrigadeControls::default(),
                    DeviceControls::Minotaur(_) => BrigadeControls::default(),
                    DeviceControls::Monarch(_) => BrigadeControls::default(),
                    DeviceControls::GodessOne(_) => BrigadeControls::default(),
                    DeviceControls::Dartford(_) => BrigadeControls::default(),
                    DeviceControls::Tron(_) => BrigadeControls::default(),
                    DeviceControls::Jetstream(_) => BrigadeControls::default(),
                    DeviceControls::Celeste(_) => BrigadeControls::default(),
                    DeviceControls::Springfield(_) => BrigadeControls::default(),
                },
            ),
            Self::Springfield(pedal) => pedal.process_loaded_voltage(
                input_voltage,
                match controls {
                    DeviceControls::Springfield(controls) => controls,
                    DeviceControls::Default => SpringfieldControls::default(),
                    DeviceControls::Lumen(_) => SpringfieldControls::default(),
                    DeviceControls::Muon(_) => SpringfieldControls::default(),
                    DeviceControls::Muffin(_) => SpringfieldControls::default(),
                    DeviceControls::Minotaur(_) => SpringfieldControls::default(),
                    DeviceControls::Monarch(_) => SpringfieldControls::default(),
                    DeviceControls::GodessOne(_) => SpringfieldControls::default(),
                    DeviceControls::Dartford(_) => SpringfieldControls::default(),
                    DeviceControls::Tron(_) => SpringfieldControls::default(),
                    DeviceControls::Jetstream(_) => SpringfieldControls::default(),
                    DeviceControls::Celeste(_) => SpringfieldControls::default(),
                    DeviceControls::Brigade(_) => SpringfieldControls::default(),
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pedal::DartfordWave;

    fn amp_controls() -> AmpControls {
        AmpControls {
            volume: 0.5,
            bass: 0.5,
            treble: 0.5,
            cut: 0.5,
            output: 1.0,
            drive: 0.0,
            presence: 0.0,
            sag: 0.0,
        }
    }

    fn chain_controls<'a>(devices: &'a [DeviceSlotControls]) -> SignalChainControls<'a> {
        SignalChainControls {
            amp: amp_controls(),
            devices,
        }
    }

    #[test]
    fn model_descriptors_define_distinct_visual_colors() {
        assert_eq!(
            DeviceConfig::Lumen.model_descriptor().visual.color,
            "silver"
        );
        assert_eq!(
            DeviceConfig::Muon.model_descriptor().visual.color,
            "seafoam"
        );
        assert_eq!(
            DeviceConfig::Muffin.model_descriptor().visual.color,
            "olive"
        );
        assert_eq!(
            DeviceConfig::Minotaur.model_descriptor().visual.color,
            "gold"
        );
        assert_eq!(
            DeviceConfig::Monarch.model_descriptor().visual.color,
            "royal-purple"
        );
        assert_eq!(
            DeviceConfig::GodessOne.model_descriptor().visual.color,
            "orange"
        );
        assert_eq!(
            DeviceConfig::Dartford.model_descriptor().visual.color,
            "teal"
        );
        assert_eq!(
            DeviceConfig::Tron.model_descriptor().visual.color,
            "lamp-orange"
        );
        assert_eq!(
            DeviceConfig::Jetstream.model_descriptor().visual.color,
            "sky-blue"
        );
        assert_eq!(
            DeviceConfig::Celeste.model_descriptor().visual.color,
            "cobalt-blue"
        );
        assert_eq!(
            DeviceConfig::Brigade.model_descriptor().visual.color,
            "violet"
        );
        assert_eq!(
            DeviceConfig::Springfield.model_descriptor().visual.color,
            "surf-green"
        );
        assert_eq!(amp_model_descriptor("nox30").visual.color, "copper");
        assert_eq!(
            amp_model_descriptor("sheriff800").visual.color,
            "black-gold"
        );
        assert_eq!(amp_model_descriptor("dumbler").visual.color, "tan");
    }

    #[test]
    fn amp_only_chain_matches_direct_amp() {
        let mut chain = SignalChain::new(48_000.0, SignalChainConfig::amp_only("nox30"));
        let mut amp = VoxAmp::new(48_000.0);
        let controls = chain_controls(&[]);

        for sample_idx in 0..2_048 {
            let input = (std::f32::consts::TAU * 220.0 * sample_idx as f32 / 48_000.0).sin() * 0.03;
            let chain_output = chain.process(input, controls);
            let amp_output = amp.process(input, controls.amp);
            assert!((chain_output - amp_output).abs() < 1e-6);
        }
    }

    #[test]
    fn pre_amp_device_changes_amp_input_path() {
        let mut clean = SignalChain::new(48_000.0, SignalChainConfig::amp_only("nox30"));
        let mut with_pedal = SignalChain::new(
            48_000.0,
            SignalChainConfig::amp_only("nox30").with_pre_amp_device(DeviceConfig::Muffin),
        );
        let device_controls = [DeviceSlotControls::active(DeviceControls::Muffin(
            MuffinControls::default(),
        ))];
        let controls = chain_controls(&device_controls);
        let mut difference_sum = 0.0;

        for sample_idx in 0..4_096 {
            let input = (std::f32::consts::TAU * 110.0 * sample_idx as f32 / 48_000.0).sin() * 0.05;
            difference_sum +=
                (clean.process(input, controls) - with_pedal.process(input, controls)).abs();
        }

        assert!(difference_sum > 1.0);
    }

    #[test]
    fn lumen_first_pre_amp_levels_hot_input_without_muting() {
        let mut bypass_config = SignalChainConfig::amp_only("nox30");
        bypass_config
            .pre_amp
            .push(DeviceSlotConfig::bypassed(DeviceConfig::Lumen));
        let mut active_config = SignalChainConfig::amp_only("nox30");
        active_config
            .pre_amp
            .push(DeviceSlotConfig::active(DeviceConfig::Lumen));
        let mut bypassed = SignalChain::new(48_000.0, bypass_config);
        let mut active = SignalChain::new(48_000.0, active_config);
        let bypassed_controls = [DeviceSlotControls::bypassed(DeviceControls::Lumen(
            LumenControls::default(),
        ))];
        let active_controls = [DeviceSlotControls::active(DeviceControls::Lumen(
            LumenControls {
                peak_reduction: 0.70,
                gain: 0.52,
                emphasis: 0.50,
                mix: 0.88,
            },
        ))];
        let mut difference_sum = 0.0;
        let mut active_sum = 0.0;

        for sample_idx in 0..48_000 {
            let amplitude = if sample_idx < 24_000 { 0.035 } else { 0.16 };
            let input =
                (std::f32::consts::TAU * 220.0 * sample_idx as f32 / 48_000.0).sin() * amplitude;
            let bypassed_output = bypassed.process(input, chain_controls(&bypassed_controls));
            let active_output = active.process(input, chain_controls(&active_controls));
            if sample_idx >= 24_000 {
                difference_sum += (active_output - bypassed_output).abs();
                active_sum += active_output.abs();
            }
        }

        assert!(difference_sum > 0.5, "difference_sum={difference_sum}");
        assert!(active_sum > 1.0, "active_sum={active_sum}");
    }

    #[test]
    fn muon_pre_amp_device_adds_envelope_filter_without_muting() {
        let mut bypass_config = SignalChainConfig::amp_only("nox30");
        bypass_config
            .pre_amp
            .push(DeviceSlotConfig::bypassed(DeviceConfig::Muon));
        let mut active_config = SignalChainConfig::amp_only("nox30");
        active_config
            .pre_amp
            .push(DeviceSlotConfig::active(DeviceConfig::Muon));
        let mut bypassed = SignalChain::new(48_000.0, bypass_config);
        let mut active = SignalChain::new(48_000.0, active_config);
        let bypassed_controls = [DeviceSlotControls::bypassed(DeviceControls::Muon(
            MuonControls::default(),
        ))];
        let active_controls = [DeviceSlotControls::active(DeviceControls::Muon(
            MuonControls {
                sensitivity: 0.68,
                range: 0.72,
                resonance: 0.58,
                mix: 0.88,
            },
        ))];
        let mut difference_sum = 0.0;
        let mut active_sum = 0.0;

        for sample_idx in 0..48_000 {
            let burst = if sample_idx % 10_000 < 2_000 {
                1.0
            } else {
                0.22
            };
            let input = ((std::f32::consts::TAU * 147.0 * sample_idx as f32 / 48_000.0).sin()
                * 0.08
                + (std::f32::consts::TAU * 441.0 * sample_idx as f32 / 48_000.0).sin() * 0.04)
                * burst;
            let bypassed_output = bypassed.process(input, chain_controls(&bypassed_controls));
            let active_output = active.process(input, chain_controls(&active_controls));
            if sample_idx >= 12_000 {
                difference_sum += (active_output - bypassed_output).abs();
                active_sum += active_output.abs();
            }
        }

        assert!(difference_sum > 0.5, "difference_sum={difference_sum}");
        assert!(active_sum > 1.0, "active_sum={active_sum}");
    }

    #[test]
    fn different_pre_amp_devices_have_distinct_paths() {
        let mut muffin = SignalChain::new(
            48_000.0,
            SignalChainConfig::amp_only("nox30").with_pre_amp_device(DeviceConfig::Muffin),
        );
        let mut minotaur = SignalChain::new(
            48_000.0,
            SignalChainConfig::amp_only("nox30").with_pre_amp_device(DeviceConfig::Minotaur),
        );
        let muffin_controls = [DeviceSlotControls::active(DeviceControls::Muffin(
            MuffinControls::default(),
        ))];
        let minotaur_controls = [DeviceSlotControls::active(DeviceControls::Minotaur(
            MinotaurControls::default(),
        ))];
        let mut difference_sum = 0.0;

        for sample_idx in 0..4_096 {
            let input = (std::f32::consts::TAU * 147.0 * sample_idx as f32 / 48_000.0).sin() * 0.05;
            let muffin_output = muffin.process(input, chain_controls(&muffin_controls));
            let minotaur_output = minotaur.process(input, chain_controls(&minotaur_controls));
            difference_sum += (muffin_output - minotaur_output).abs();
        }

        assert!(difference_sum > 0.1);
    }

    #[test]
    fn amp_bypass_preserves_active_pre_amp_pedals() {
        let mut chain = SignalChain::new(
            48_000.0,
            SignalChainConfig::amp_only("nox30").with_pre_amp_device(DeviceConfig::Minotaur),
        );
        let controls = [DeviceSlotControls::active(DeviceControls::Minotaur(
            MinotaurControls {
                gain: 0.75,
                treble: 0.55,
                output: 0.65,
            },
        ))];
        let mut difference_sum = 0.0;

        for sample_idx in 0..9_600 {
            let input =
                (std::f32::consts::TAU * 220.0 * sample_idx as f32 / 48_000.0).sin() * 0.12;
            let output =
                chain.process_with_amp_enabled(input, chain_controls(&controls), false);
            if sample_idx >= 4_800 {
                difference_sum += (output - input).abs();
            }
        }

        assert!(difference_sum > 1.0, "difference_sum={difference_sum}");
    }

    #[test]
    fn fx_loop_device_modulates_amp_send_before_power_stage() {
        let mut clean = SignalChain::new(48_000.0, SignalChainConfig::amp_only("nox30"));
        let mut config = SignalChainConfig::amp_only("nox30");
        config
            .fx_loop
            .push(DeviceSlotConfig::active(DeviceConfig::Dartford));
        let mut with_dartford = SignalChain::new(48_000.0, config);
        let device_controls = [DeviceSlotControls::active(DeviceControls::Dartford(
            DartfordControls {
                rate_hz: 6.0,
                depth: 0.46,
                level: 1.0,
                wave: DartfordWave::Sine,
            },
        ))];
        let clean_controls = chain_controls(&[]);
        let dartford_controls = chain_controls(&device_controls);
        let mut difference_sum = 0.0;
        let mut dartford_output_sum = 0.0;

        for sample_idx in 0..48_000 {
            let input = (std::f32::consts::TAU * 147.0 * sample_idx as f32 / 48_000.0).sin() * 0.06;
            let clean_output = clean.process(input, clean_controls);
            let dartford_output = with_dartford.process(input, dartford_controls);
            if sample_idx >= 24_000 {
                difference_sum += (clean_output - dartford_output).abs();
                dartford_output_sum += dartford_output.abs();
            }
        }

        assert!(difference_sum > 1.0, "difference_sum={difference_sum}");
        assert!(
            dartford_output_sum > 5.0,
            "dartford_output_sum={dartford_output_sum}"
        );
    }

    #[test]
    fn tron_pre_amp_device_changes_phase_motion_without_muting() {
        let mut bypass_config = SignalChainConfig::amp_only("nox30");
        bypass_config
            .pre_amp
            .push(DeviceSlotConfig::bypassed(DeviceConfig::Tron));
        let mut active_config = SignalChainConfig::amp_only("nox30");
        active_config
            .pre_amp
            .push(DeviceSlotConfig::active(DeviceConfig::Tron));
        let mut bypassed = SignalChain::new(48_000.0, bypass_config);
        let mut active = SignalChain::new(48_000.0, active_config);
        let bypassed_controls = [DeviceSlotControls::bypassed(DeviceControls::Tron(
            TronControls::default(),
        ))];
        let active_controls = [DeviceSlotControls::active(DeviceControls::Tron(
            TronControls {
                rate_hz: 0.9,
                depth: 0.74,
                feedback: 0.38,
                mix: 0.62,
            },
        ))];
        let mut difference_sum = 0.0;
        let mut active_output_sum = 0.0;
        let mut bypassed_energy = 0.0;
        let mut active_energy = 0.0;

        for sample_idx in 0..48_000 {
            let input = (std::f32::consts::TAU * 196.0 * sample_idx as f32 / 48_000.0).sin() * 0.05
                + (std::f32::consts::TAU * 622.0 * sample_idx as f32 / 48_000.0).sin() * 0.025;
            let bypassed_output = bypassed.process(input, chain_controls(&bypassed_controls));
            let active_output = active.process(input, chain_controls(&active_controls));
            if sample_idx >= 24_000 {
                difference_sum += (active_output - bypassed_output).abs();
                active_output_sum += active_output.abs();
                bypassed_energy += bypassed_output * bypassed_output;
                active_energy += active_output * active_output;
            }
        }

        assert!(difference_sum > 0.5, "difference_sum={difference_sum}");
        assert!(
            active_output_sum > 1.0,
            "active_output_sum={active_output_sum}"
        );
        assert!(
            active_energy > bypassed_energy * 0.45,
            "active_energy={active_energy}, bypassed_energy={bypassed_energy}"
        );
        assert!(
            active_energy < bypassed_energy * 1.30,
            "active_energy={active_energy}, bypassed_energy={bypassed_energy}"
        );
    }

    #[test]
    fn jetstream_pre_amp_device_sweeps_comb_without_muting() {
        let mut bypass_config = SignalChainConfig::amp_only("nox30");
        bypass_config
            .pre_amp
            .push(DeviceSlotConfig::bypassed(DeviceConfig::Jetstream));
        let mut active_config = SignalChainConfig::amp_only("nox30");
        active_config
            .pre_amp
            .push(DeviceSlotConfig::active(DeviceConfig::Jetstream));
        let mut bypassed = SignalChain::new(48_000.0, bypass_config);
        let mut active = SignalChain::new(48_000.0, active_config);
        let bypassed_controls = [DeviceSlotControls::bypassed(DeviceControls::Jetstream(
            JetstreamControls::default(),
        ))];
        let active_controls = [DeviceSlotControls::active(DeviceControls::Jetstream(
            JetstreamControls {
                manual: 0.44,
                rate_hz: 0.32,
                depth: 0.78,
                feedback: 0.52,
                mix: 0.62,
            },
        ))];
        let mut difference_sum = 0.0;
        let mut active_output_sum = 0.0;

        for sample_idx in 0..48_000 {
            let input = (std::f32::consts::TAU * 196.0 * sample_idx as f32 / 48_000.0).sin() * 0.05
                + (std::f32::consts::TAU * 740.0 * sample_idx as f32 / 48_000.0).sin() * 0.03;
            let bypassed_output = bypassed.process(input, chain_controls(&bypassed_controls));
            let active_output = active.process(input, chain_controls(&active_controls));
            if sample_idx >= 24_000 {
                difference_sum += (active_output - bypassed_output).abs();
                active_output_sum += active_output.abs();
            }
        }

        assert!(difference_sum > 0.5, "difference_sum={difference_sum}");
        assert!(
            active_output_sum > 1.0,
            "active_output_sum={active_output_sum}"
        );
    }

    #[test]
    fn celeste_pre_amp_device_adds_chorus_motion_without_muting() {
        let mut bypass_config = SignalChainConfig::amp_only("nox30");
        bypass_config
            .pre_amp
            .push(DeviceSlotConfig::bypassed(DeviceConfig::Celeste));
        let mut active_config = SignalChainConfig::amp_only("nox30");
        active_config
            .pre_amp
            .push(DeviceSlotConfig::active(DeviceConfig::Celeste));
        let mut bypassed = SignalChain::new(48_000.0, bypass_config);
        let mut active = SignalChain::new(48_000.0, active_config);
        let bypassed_controls = [DeviceSlotControls::bypassed(DeviceControls::Celeste(
            CelesteControls::default(),
        ))];
        let active_controls = [DeviceSlotControls::active(DeviceControls::Celeste(
            CelesteControls {
                rate_hz: 0.72,
                depth: 0.72,
                tone: 0.58,
                mix: 0.48,
            },
        ))];
        let mut difference_sum = 0.0;
        let mut active_output_sum = 0.0;

        for sample_idx in 0..48_000 {
            let input = (std::f32::consts::TAU * 220.0 * sample_idx as f32 / 48_000.0).sin() * 0.05
                + (std::f32::consts::TAU * 660.0 * sample_idx as f32 / 48_000.0).sin() * 0.03;
            let bypassed_output = bypassed.process(input, chain_controls(&bypassed_controls));
            let active_output = active.process(input, chain_controls(&active_controls));
            if sample_idx >= 24_000 {
                difference_sum += (active_output - bypassed_output).abs();
                active_output_sum += active_output.abs();
            }
        }

        assert!(difference_sum > 0.5, "difference_sum={difference_sum}");
        assert!(
            active_output_sum > 1.0,
            "active_output_sum={active_output_sum}"
        );
    }

    #[test]
    fn springfield_fx_loop_stays_audible_after_tank_settles() {
        let mut config = SignalChainConfig::amp_only("nox30");
        config
            .fx_loop
            .push(DeviceSlotConfig::active(DeviceConfig::Springfield));
        let mut chain = SignalChain::new(44_100.0, config);
        let mut clean = SignalChain::new(44_100.0, SignalChainConfig::amp_only("nox30"));
        let mut bypass_config = SignalChainConfig::amp_only("nox30");
        bypass_config
            .fx_loop
            .push(DeviceSlotConfig::bypassed(DeviceConfig::Springfield));
        let mut bypassed = SignalChain::new(44_100.0, bypass_config);
        let device_controls = [DeviceSlotControls::active(DeviceControls::Springfield(
            SpringfieldControls {
                dwell: 0.48,
                tone: 0.58,
                mix: 0.26,
            },
        ))];
        let controls = SignalChainControls {
            amp: AmpControls {
                volume: 0.56,
                bass: 0.56,
                treble: 0.58,
                cut: 0.44,
                output: 10.0_f32.powf(-9.0 / 20.0),
                drive: 0.24,
                presence: 0.34,
                sag: 0.46,
            },
            devices: &device_controls,
        };
        let mut settled_output_sum = 0.0;
        let mut clean_settled_output_sum = 0.0;
        let mut bypassed_settled_output_sum = 0.0;
        let mut active_bypassed_difference_sum = 0.0;

        for sample_idx in 0..88_200 {
            let input = (std::f32::consts::TAU * 147.0 * sample_idx as f32 / 44_100.0).sin() * 0.06
                + (std::f32::consts::TAU * 220.0 * sample_idx as f32 / 44_100.0).sin() * 0.03;
            let output = chain.process(input, controls);
            let clean_output = clean.process(
                input,
                SignalChainControls {
                    amp: controls.amp,
                    devices: &[],
                },
            );
            let bypassed_output = bypassed.process(
                input,
                SignalChainControls {
                    amp: controls.amp,
                    devices: &[DeviceSlotControls::bypassed(DeviceControls::Springfield(
                        SpringfieldControls {
                            dwell: 0.48,
                            tone: 0.58,
                            mix: 0.0,
                        },
                    ))],
                },
            );
            assert!(output.is_finite(), "output={output}");
            if sample_idx >= 44_100 {
                settled_output_sum += output.abs();
                clean_settled_output_sum += clean_output.abs();
                bypassed_settled_output_sum += bypassed_output.abs();
                active_bypassed_difference_sum += (output - bypassed_output).abs();
            }
        }

        assert!(
            clean_settled_output_sum > 1.0,
            "clean_settled_output_sum={clean_settled_output_sum}"
        );
        assert!(
            settled_output_sum > 1.0,
            "settled_output_sum={settled_output_sum}, clean_settled_output_sum={clean_settled_output_sum}, bypassed_settled_output_sum={bypassed_settled_output_sum}"
        );
        assert!(
            active_bypassed_difference_sum > 1.5,
            "active_bypassed_difference_sum={active_bypassed_difference_sum}"
        );
    }

    #[test]
    fn bypassed_slot_preserves_audio_path_shape() {
        let mut clean = SignalChain::new(48_000.0, SignalChainConfig::amp_only("nox30"));
        let mut config = SignalChainConfig::amp_only("nox30");
        config
            .pre_amp
            .push(DeviceSlotConfig::bypassed(DeviceConfig::Muffin));
        let mut bypassed = SignalChain::new(48_000.0, config);
        let controls = chain_controls(&[]);

        for sample_idx in 0..2_048 {
            let input = (std::f32::consts::TAU * 330.0 * sample_idx as f32 / 48_000.0).sin() * 0.02;
            let clean_output = clean.process(input, controls);
            let bypassed_output = bypassed.process(input, controls);
            assert!((clean_output - bypassed_output).abs() < 1e-6);
        }
    }

    #[test]
    fn brigade_fx_loop_adds_dark_repeats_without_muting() {
        let mut config = SignalChainConfig::amp_only("nox30");
        config
            .fx_loop
            .push(DeviceSlotConfig::active(DeviceConfig::Brigade));
        let mut chain = SignalChain::new(44_100.0, config);
        let mut clean = SignalChain::new(44_100.0, SignalChainConfig::amp_only("nox30"));
        let device_controls = [DeviceSlotControls::active(DeviceControls::Brigade(
            BrigadeControls {
                time_ms: 180.0,
                repeats: 0.54,
                tone: 0.38,
                mix: 0.34,
            },
        ))];
        let clean_controls = chain_controls(&[]);
        let brigade_controls = chain_controls(&device_controls);
        let mut difference_sum = 0.0;
        let mut active_output_sum = 0.0;

        for sample_idx in 0..44_100 {
            let input = if sample_idx < 900 {
                (std::f32::consts::TAU * 147.0 * sample_idx as f32 / 44_100.0).sin() * 0.08
            } else {
                0.0
            };
            let clean_output = clean.process(input, clean_controls);
            let active_output = chain.process(input, brigade_controls);
            if sample_idx >= 12_000 {
                difference_sum += (active_output - clean_output).abs();
                active_output_sum += active_output.abs();
            }
        }

        assert!(difference_sum > 0.2, "difference_sum={difference_sum}");
        assert!(
            active_output_sum > 0.2,
            "active_output_sum={active_output_sum}"
        );
    }

    #[test]
    fn slot_controls_are_indexed_by_chain_order() {
        let mut config = SignalChainConfig::amp_only("nox30")
            .with_pre_amp_device(DeviceConfig::Muffin)
            .with_pre_amp_device(DeviceConfig::Muffin);
        config.cable_capacitance_farads = 0.0;
        let mut first_active = SignalChain::new(48_000.0, config.clone());
        let mut second_active = SignalChain::new(48_000.0, config);
        let first_controls = [
            DeviceSlotControls::active(DeviceControls::Muffin(MuffinControls {
                sustain: 0.9,
                tone: 0.2,
                level: 0.7,
            })),
            DeviceSlotControls::bypassed(DeviceControls::Default),
        ];
        let second_controls = [
            DeviceSlotControls::bypassed(DeviceControls::Default),
            DeviceSlotControls::active(DeviceControls::Muffin(MuffinControls {
                sustain: 0.1,
                tone: 0.8,
                level: 0.7,
            })),
        ];
        let mut difference_sum = 0.0;

        for sample_idx in 0..4_096 {
            let input = (std::f32::consts::TAU * 147.0 * sample_idx as f32 / 48_000.0).sin() * 0.04;
            let first = first_active.process(input, chain_controls(&first_controls));
            let second = second_active.process(input, chain_controls(&second_controls));
            difference_sum += (first - second).abs();
        }

        assert!(difference_sum > 0.1);
    }
}
