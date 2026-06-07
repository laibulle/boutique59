use crate::amp::AmpControls;
use crate::chain::{
    DeviceConfig, DeviceControls, DeviceSlotConfig, DeviceSlotControls, SignalChainConfig,
};
use crate::pedal::{
    BrigadeControls, CelesteControls, DartfordControls, DartfordWave, GodessOneControls,
    GodessOneMode, JetstreamControls, LumenControls, MinotaurControls, MonarchControls,
    MuffinControls, MuonControls, SpringfieldControls, TronControls,
};
use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigConfig {
    pub name: Option<String>,
    #[serde(default)]
    pub chain: RigChainOptions,
    pub amp: RigAmpConfig,
    #[serde(default)]
    pub pre_amp: Vec<RigDeviceSlot>,
    #[serde(default)]
    pub fx_loop: Vec<RigDeviceSlot>,
    #[serde(default)]
    pub post_amp: Vec<RigDeviceSlot>,
    pub cab: Option<RigCabConfig>,
}

impl RigConfig {
    pub fn from_json5(text: &str) -> Result<Self> {
        json5::from_str(text).map_err(Into::into)
    }

    pub fn signal_chain_config(&self) -> Result<SignalChainConfig> {
        let mut config = SignalChainConfig::amp_only(&self.amp.model);
        if let Some(cable_capacitance_pf) = self.chain.cable_capacitance_pf {
            config.cable_capacitance_farads = cable_capacitance_pf.max(0.0) * 1e-12;
        }
        config.pre_amp = self
            .pre_amp
            .iter()
            .map(RigDeviceSlot::device_slot_config)
            .collect::<Result<Vec<_>>>()?;
        config.fx_loop = self
            .fx_loop
            .iter()
            .map(RigDeviceSlot::device_slot_config)
            .collect::<Result<Vec<_>>>()?;
        config.post_amp = self
            .post_amp
            .iter()
            .map(RigDeviceSlot::device_slot_config)
            .collect::<Result<Vec<_>>>()?;
        Ok(config)
    }

    pub fn amp_controls(&self, output_gain: f32) -> AmpControls {
        self.amp.controls.to_amp_controls(output_gain)
    }

    pub fn amp_enabled(&self) -> bool {
        !self.amp.bypassed
    }

    pub fn cab_ir_enabled(&self) -> Result<bool> {
        let Some(cab) = &self.cab else {
            return Ok(false);
        };
        if !matches!(cab.ir.as_str(), "tone3000-celestion" | "embedded-v30") {
            bail!("unknown cab IR '{}'", cab.ir);
        }
        Ok(!cab.bypassed)
    }

    pub fn device_controls(&self) -> Result<Vec<DeviceSlotControls>> {
        self.pre_amp
            .iter()
            .chain(self.fx_loop.iter())
            .chain(self.post_amp.iter())
            .map(RigDeviceSlot::device_slot_controls)
            .collect()
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigChainOptions {
    pub cable_capacitance_pf: Option<f32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigAmpConfig {
    pub model: String,
    #[serde(default)]
    pub bypassed: bool,
    #[serde(default)]
    pub controls: RigAmpControls,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigCabConfig {
    pub ir: String,
    #[serde(default)]
    pub bypassed: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RigAmpControls {
    pub volume: f32,
    pub bass: f32,
    pub treble: f32,
    pub cut: f32,
    pub drive: f32,
    pub presence: f32,
    pub sag: f32,
}

impl Default for RigAmpControls {
    fn default() -> Self {
        Self {
            volume: 0.55,
            bass: 0.5,
            treble: 0.6,
            cut: 0.35,
            drive: 0.0,
            presence: 0.0,
            sag: 0.0,
        }
    }
}

impl RigAmpControls {
    fn to_amp_controls(self, output_gain: f32) -> AmpControls {
        AmpControls {
            volume: self.volume.clamp(0.0, 1.0),
            bass: self.bass.clamp(0.0, 1.0),
            treble: self.treble.clamp(0.0, 1.0),
            cut: self.cut.clamp(0.0, 1.0),
            output: output_gain,
            drive: self.drive.clamp(0.0, 1.0),
            presence: self.presence.clamp(0.0, 1.0),
            sag: self.sag.clamp(0.0, 1.0),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigDeviceSlot {
    pub id: Option<String>,
    pub device: String,
    #[serde(default)]
    pub bypassed: bool,
    #[serde(default)]
    pub controls: RigDeviceControls,
}

impl RigDeviceSlot {
    fn device_slot_config(&self) -> Result<DeviceSlotConfig> {
        let device = parse_device_config(&self.device)?;
        Ok(DeviceSlotConfig {
            device,
            bypassed: self.bypassed,
        })
    }

    fn device_slot_controls(&self) -> Result<DeviceSlotControls> {
        let controls = match parse_device_config(&self.device)? {
            DeviceConfig::Lumen => DeviceControls::Lumen(self.controls.lumen()),
            DeviceConfig::Muon => DeviceControls::Muon(self.controls.muon()),
            DeviceConfig::Muffin => DeviceControls::Muffin(self.controls.muffin()),
            DeviceConfig::Minotaur => DeviceControls::Minotaur(self.controls.minotaur()),
            DeviceConfig::Monarch => DeviceControls::Monarch(self.controls.monarch()),
            DeviceConfig::GodessOne => DeviceControls::GodessOne(self.controls.godess_one()),
            DeviceConfig::Dartford => DeviceControls::Dartford(self.controls.dartford()),
            DeviceConfig::Tron => DeviceControls::Tron(self.controls.tron()),
            DeviceConfig::Jetstream => DeviceControls::Jetstream(self.controls.jetstream()),
            DeviceConfig::Celeste => DeviceControls::Celeste(self.controls.celeste()),
            DeviceConfig::Brigade => DeviceControls::Brigade(self.controls.brigade()),
            DeviceConfig::Springfield => DeviceControls::Springfield(self.controls.springfield()),
        };
        Ok(DeviceSlotControls {
            bypassed: self.bypassed,
            controls,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RigDeviceControls {
    pub peak_reduction: f32,
    pub sensitivity: f32,
    pub range: f32,
    pub resonance: f32,
    pub sustain: f32,
    pub tone: f32,
    pub level: f32,
    pub gain: f32,
    pub emphasis: f32,
    pub treble: f32,
    pub output: f32,
    pub distortion: f32,
    pub mode: GodessOneMode,
    pub manual: f32,
    pub time_ms: f32,
    pub rate_hz: f32,
    pub depth: f32,
    pub feedback: f32,
    pub repeats: f32,
    pub wave: DartfordWave,
    pub dwell: f32,
    pub mix: f32,
}

impl Default for RigDeviceControls {
    fn default() -> Self {
        let lumen = LumenControls::default();
        let muon = MuonControls::default();
        let muffin = MuffinControls::default();
        let minotaur = MinotaurControls::default();
        let godess_one = GodessOneControls::default();
        let dartford = DartfordControls::default();
        let tron = TronControls::default();
        let jetstream = JetstreamControls::default();
        let brigade = BrigadeControls::default();
        let springfield = SpringfieldControls::default();
        Self {
            peak_reduction: lumen.peak_reduction,
            sensitivity: muon.sensitivity,
            range: muon.range,
            resonance: muon.resonance,
            sustain: muffin.sustain,
            tone: muffin.tone,
            level: muffin.level,
            gain: minotaur.gain,
            emphasis: lumen.emphasis,
            treble: minotaur.treble,
            output: minotaur.output,
            distortion: godess_one.distortion,
            mode: godess_one.mode,
            manual: jetstream.manual,
            time_ms: brigade.time_ms,
            rate_hz: dartford.rate_hz,
            depth: dartford.depth,
            feedback: tron.feedback,
            repeats: brigade.repeats,
            wave: dartford.wave,
            dwell: springfield.dwell,
            mix: springfield.mix,
        }
    }
}

impl RigDeviceControls {
    fn lumen(self) -> LumenControls {
        LumenControls {
            peak_reduction: self.peak_reduction.clamp(0.0, 1.0),
            gain: self.gain.clamp(0.0, 1.0),
            emphasis: self.emphasis.clamp(0.0, 1.0),
            mix: self.mix.clamp(0.0, 1.0),
        }
    }

    fn muon(self) -> MuonControls {
        MuonControls {
            sensitivity: self.sensitivity.clamp(0.0, 1.0),
            range: self.range.clamp(0.0, 1.0),
            resonance: self.resonance.clamp(0.0, 1.0),
            mix: self.mix.clamp(0.0, 1.0),
        }
    }

    fn muffin(self) -> MuffinControls {
        MuffinControls {
            sustain: self.sustain.clamp(0.0, 1.0),
            tone: self.tone.clamp(0.0, 1.0),
            level: self.level.clamp(0.0, 1.0),
        }
    }

    fn minotaur(self) -> MinotaurControls {
        MinotaurControls {
            gain: self.gain.clamp(0.0, 1.0),
            treble: self.treble.clamp(0.0, 1.0),
            output: self.output.clamp(0.0, 1.0),
        }
    }

    fn monarch(self) -> MonarchControls {
        MonarchControls {
            gain: self.gain.clamp(0.0, 1.0),
            tone: self.tone.clamp(0.0, 1.0),
            output: self.output.clamp(0.0, 1.0),
        }
    }

    fn godess_one(self) -> GodessOneControls {
        GodessOneControls {
            distortion: self.distortion.clamp(0.0, 1.0),
            tone: self.tone.clamp(0.0, 1.0),
            level: self.level.clamp(0.0, 1.0),
            mode: self.mode,
        }
    }

    fn dartford(self) -> DartfordControls {
        DartfordControls {
            rate_hz: self.rate_hz.clamp(0.05, 20.0),
            depth: self.depth.clamp(0.0, 1.0),
            level: self.level.clamp(0.0, 2.0),
            wave: self.wave,
        }
    }

    fn tron(self) -> TronControls {
        TronControls {
            rate_hz: self.rate_hz.clamp(0.03, 12.0),
            depth: self.depth.clamp(0.0, 1.0),
            feedback: self.feedback.clamp(0.0, 0.92),
            mix: self.mix.clamp(0.0, 1.0),
        }
    }

    fn jetstream(self) -> JetstreamControls {
        JetstreamControls {
            manual: self.manual.clamp(0.0, 1.0),
            rate_hz: self.rate_hz.clamp(0.02, 8.0),
            depth: self.depth.clamp(0.0, 1.0),
            feedback: self.feedback.clamp(0.0, 0.94),
            mix: self.mix.clamp(0.0, 1.0),
        }
    }

    fn celeste(self) -> CelesteControls {
        CelesteControls {
            rate_hz: self.rate_hz.clamp(0.05, 6.0),
            depth: self.depth.clamp(0.0, 1.0),
            tone: self.tone.clamp(0.0, 1.0),
            mix: self.mix.clamp(0.0, 1.0),
        }
    }

    fn brigade(self) -> BrigadeControls {
        BrigadeControls {
            time_ms: self.time_ms.clamp(60.0, 700.0),
            repeats: self.repeats.clamp(0.0, 0.92),
            tone: self.tone.clamp(0.0, 1.0),
            mix: self.mix.clamp(0.0, 1.0),
        }
    }

    fn springfield(self) -> SpringfieldControls {
        SpringfieldControls {
            dwell: self.dwell.clamp(0.0, 1.0),
            tone: self.tone.clamp(0.0, 1.0),
            mix: self.mix.clamp(0.0, 1.0),
        }
    }
}

fn parse_device_config(device: &str) -> Result<DeviceConfig> {
    match device {
        "lumen" => Ok(DeviceConfig::Lumen),
        "muon" => Ok(DeviceConfig::Muon),
        "muffin" => Ok(DeviceConfig::Muffin),
        "minotaur" => Ok(DeviceConfig::Minotaur),
        "monarch" => Ok(DeviceConfig::Monarch),
        "godess-one" => Ok(DeviceConfig::GodessOne),
        "dartford" => Ok(DeviceConfig::Dartford),
        "tron" => Ok(DeviceConfig::Tron),
        "jetstream" => Ok(DeviceConfig::Jetstream),
        "celeste" => Ok(DeviceConfig::Celeste),
        "brigade" => Ok(DeviceConfig::Brigade),
        "springfield" => Ok(DeviceConfig::Springfield),
        _ => bail!("unknown rig device '{device}'"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json5_rig_with_comments_and_unquoted_keys() {
        let rig = RigConfig::from_json5(
            r#"
            {
              // Runtime I/O intentionally does not belong here.
              name: 'muffin-nox30',
              chain: { cable_capacitance_pf: 470 },
              pre_amp: [{
                id: 'fuzz',
                device: 'muffin',
                bypassed: false,
                controls: { sustain: 0.7, tone: 0.46, level: 0.45 },
              }],
              amp: {
                model: 'nox30',
                controls: { volume: 0.48, bass: 0.52, treble: 0.58, cut: 0.46, drive: 0.25, presence: 0.32, sag: 0.55 },
              },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("muffin-nox30"));
        assert_eq!(chain.amp_model, "nox30");
        assert_eq!(chain.pre_amp.len(), 1);
        assert_eq!(controls.len(), 1);
        assert!((chain.cable_capacitance_farads - 470e-12).abs() < 1e-15);
    }

    #[test]
    fn parses_explicit_bypass_and_cab_state() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'nox30-all-pedals-bypassed',
              pre_amp: [{
                id: 'fuzz',
                device: 'muffin',
                bypassed: true,
              }],
              amp: {
                model: 'nox30',
                bypassed: false,
              },
              cab: {
                ir: 'tone3000-celestion',
                bypassed: false,
              },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(
            chain.pre_amp,
            vec![DeviceSlotConfig::bypassed(DeviceConfig::Muffin)]
        );
        assert!(controls[0].bypassed);
        assert!(rig.amp_enabled());
        assert!(rig.cab_ir_enabled().unwrap());
    }

    #[test]
    fn parses_full_bypassed_pedalboard_fixture() {
        let rig = RigConfig::from_json5(include_str!("../../rigs/nox30-all-pedals-bypassed.json5"))
            .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("nox30-all-pedals-bypassed"));
        assert_eq!(chain.pre_amp.len(), 8);
        assert_eq!(chain.fx_loop.len(), 4);
        assert!(chain.pre_amp.iter().all(|slot| slot.bypassed));
        assert!(chain.fx_loop.iter().all(|slot| slot.bypassed));
        assert_eq!(controls.len(), 12);
        assert!(controls.iter().all(|slot| slot.bypassed));
        assert!(rig.amp_enabled());
        assert!(rig.cab_ir_enabled().unwrap());
    }

    #[test]
    fn rejects_runtime_io_fields() {
        let error = RigConfig::from_json5(
            r#"
            {
              input: { device: 'headphones' },
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn parses_minotaur_rig_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'minotaur-nox30',
              pre_amp: [{
                id: 'overdrive',
                device: 'minotaur',
                controls: { gain: 0.42, treble: 0.61, output: 0.58 },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("minotaur-nox30"));
        assert_eq!(
            chain.pre_amp,
            vec![DeviceSlotConfig::active(DeviceConfig::Minotaur)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Minotaur(MinotaurControls {
                gain,
                treble,
                output
            }) if (gain - 0.42).abs() < 1e-6
                && (treble - 0.61).abs() < 1e-6
                && (output - 0.58).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_lumen_compressor_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'lumen-nox30',
              pre_amp: [{
                id: 'opto-compressor',
                device: 'lumen',
                controls: { peak_reduction: 0.66, gain: 0.52, emphasis: 0.48, mix: 0.86 },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("lumen-nox30"));
        assert_eq!(
            chain.pre_amp,
            vec![DeviceSlotConfig::active(DeviceConfig::Lumen)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Lumen(LumenControls {
                peak_reduction,
                gain,
                emphasis,
                mix,
            }) if (peak_reduction - 0.66).abs() < 1e-6
                && (gain - 0.52).abs() < 1e-6
                && (emphasis - 0.48).abs() < 1e-6
                && (mix - 0.86).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_muon_filter_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'muon-nox30',
              pre_amp: [{
                id: 'envelope-filter',
                device: 'muon',
                controls: { sensitivity: 0.64, range: 0.68, resonance: 0.52, mix: 0.86 },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("muon-nox30"));
        assert_eq!(
            chain.pre_amp,
            vec![DeviceSlotConfig::active(DeviceConfig::Muon)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Muon(MuonControls {
                sensitivity,
                range,
                resonance,
                mix,
            }) if (sensitivity - 0.64).abs() < 1e-6
                && (range - 0.68).abs() < 1e-6
                && (resonance - 0.52).abs() < 1e-6
                && (mix - 0.86).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_monarch_rig_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'monarch-nox30',
              pre_amp: [{
                id: 'dual-drive',
                device: 'monarch',
                controls: { gain: 0.48, tone: 0.57, output: 0.62 },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("monarch-nox30"));
        assert_eq!(
            chain.pre_amp,
            vec![DeviceSlotConfig::active(DeviceConfig::Monarch)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Monarch(MonarchControls {
                gain,
                tone,
                output
            }) if (gain - 0.48).abs() < 1e-6
                && (tone - 0.57).abs() < 1e-6
                && (output - 0.62).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_godess_one_rig_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'godess-one-nox30',
              pre_amp: [{
                id: 'distortion',
                device: 'godess-one',
                controls: { distortion: 0.64, tone: 0.47, level: 0.52, mode: 'custom' },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("godess-one-nox30"));
        assert_eq!(
            chain.pre_amp,
            vec![DeviceSlotConfig::active(DeviceConfig::GodessOne)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::GodessOne(GodessOneControls {
                distortion,
                tone,
                level,
                mode: GodessOneMode::Custom
            }) if (distortion - 0.64).abs() < 1e-6
                && (tone - 0.47).abs() < 1e-6
                && (level - 0.52).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_dartford_fx_loop_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'dartford-nox30',
              fx_loop: [{
                id: 'amp-trem',
                device: 'dartford',
                controls: { rate_hz: 5.5, depth: 0.72, level: 0.95, wave: 'triangle' },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("dartford-nox30"));
        assert_eq!(
            chain.fx_loop,
            vec![DeviceSlotConfig::active(DeviceConfig::Dartford)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Dartford(DartfordControls {
                rate_hz,
                depth,
                level,
                wave: DartfordWave::Triangle
            }) if (rate_hz - 5.5).abs() < 1e-6
                && (depth - 0.72).abs() < 1e-6
                && (level - 0.95).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_springfield_reverb_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'springfield-nox30',
              fx_loop: [{
                id: 'spring-reverb',
                device: 'springfield',
                controls: { dwell: 0.48, tone: 0.62, mix: 0.28 },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("springfield-nox30"));
        assert_eq!(
            chain.fx_loop,
            vec![DeviceSlotConfig::active(DeviceConfig::Springfield)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Springfield(SpringfieldControls {
                dwell,
                tone,
                mix,
            }) if (dwell - 0.48).abs() < 1e-6
                && (tone - 0.62).abs() < 1e-6
                && (mix - 0.28).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_tron_phaser_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'tron-nox30',
              pre_amp: [{
                id: 'organic-phaser',
                device: 'tron',
                controls: { rate_hz: 0.72, depth: 0.76, feedback: 0.42, mix: 0.64 },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("tron-nox30"));
        assert_eq!(
            chain.pre_amp,
            vec![DeviceSlotConfig::active(DeviceConfig::Tron)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Tron(TronControls {
                rate_hz,
                depth,
                feedback,
                mix,
            }) if (rate_hz - 0.72).abs() < 1e-6
                && (depth - 0.76).abs() < 1e-6
                && (feedback - 0.42).abs() < 1e-6
                && (mix - 0.64).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_jetstream_flanger_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'jetstream-nox30',
              pre_amp: [{
                id: 'bbd-flanger',
                device: 'jetstream',
                controls: { manual: 0.44, rate_hz: 0.32, depth: 0.78, feedback: 0.52, mix: 0.62 },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("jetstream-nox30"));
        assert_eq!(
            chain.pre_amp,
            vec![DeviceSlotConfig::active(DeviceConfig::Jetstream)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Jetstream(JetstreamControls {
                manual,
                rate_hz,
                depth,
                feedback,
                mix,
            }) if (manual - 0.44).abs() < 1e-6
                && (rate_hz - 0.32).abs() < 1e-6
                && (depth - 0.78).abs() < 1e-6
                && (feedback - 0.52).abs() < 1e-6
                && (mix - 0.62).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_celeste_chorus_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'celeste-nox30',
              pre_amp: [{
                id: 'analog-chorus',
                device: 'celeste',
                controls: { rate_hz: 0.72, depth: 0.72, tone: 0.58, mix: 0.48 },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("celeste-nox30"));
        assert_eq!(
            chain.pre_amp,
            vec![DeviceSlotConfig::active(DeviceConfig::Celeste)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Celeste(CelesteControls {
                rate_hz,
                depth,
                tone,
                mix,
            }) if (rate_hz - 0.72).abs() < 1e-6
                && (depth - 0.72).abs() < 1e-6
                && (tone - 0.58).abs() < 1e-6
                && (mix - 0.48).abs() < 1e-6
        ));
    }

    #[test]
    fn parses_brigade_delay_controls() {
        let rig = RigConfig::from_json5(
            r#"
            {
              name: 'brigade-nox30',
              fx_loop: [{
                id: 'analog-delay',
                device: 'brigade',
                controls: { time_ms: 320.0, repeats: 0.46, tone: 0.38, mix: 0.34 },
              }],
              amp: { model: 'nox30' },
            }
            "#,
        )
        .unwrap();

        let chain = rig.signal_chain_config().unwrap();
        let controls = rig.device_controls().unwrap();

        assert_eq!(rig.name.as_deref(), Some("brigade-nox30"));
        assert_eq!(
            chain.fx_loop,
            vec![DeviceSlotConfig::active(DeviceConfig::Brigade)]
        );
        assert!(matches!(
            controls[0].controls,
            DeviceControls::Brigade(BrigadeControls {
                time_ms,
                repeats,
                tone,
                mix,
            }) if (time_ms - 320.0).abs() < 1e-6
                && (repeats - 0.46).abs() < 1e-6
                && (tone - 0.38).abs() < 1e-6
                && (mix - 0.34).abs() < 1e-6
        ));
    }
}
