use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, Device, SampleFormat, SampleRate, StreamConfig, SupportedStreamConfigRange,
};
use rtrb::RingBuffer;
use std::env;
use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;
use voxbox::amp::{AmpControls, VoxAmp};
use voxbox::ir::SpeakerStage;

const RMS_SCALE: f64 = 1_000_000_000.0;
const NEAR_CLIP_LEVEL: f32 = 0.98;
const CLIP_LEVEL: f32 = 1.0;

#[derive(Default)]
struct MonitorStats {
    input_sum_squares: AtomicU64,
    output_sum_squares: AtomicU64,
    input_count: AtomicU64,
    output_count: AtomicU64,
    input_peak_bits: AtomicU32,
    output_peak_bits: AtomicU32,
    input_near_clips: AtomicU64,
    output_near_clips: AtomicU64,
    input_clips: AtomicU64,
    output_clips: AtomicU64,
    input_overruns: AtomicU64,
    output_underruns: AtomicU64,
}

#[derive(Default)]
struct MonitorSnapshot {
    input_sum_squares: u64,
    output_sum_squares: u64,
    input_count: u64,
    output_count: u64,
    input_peak: f32,
    output_peak: f32,
    input_near_clips: u64,
    output_near_clips: u64,
    input_clips: u64,
    output_clips: u64,
    input_overruns: u64,
    output_underruns: u64,
}

impl MonitorStats {
    fn record_input(&self, sample: f32) {
        self.record_sample(
            sample,
            &self.input_sum_squares,
            &self.input_count,
            &self.input_peak_bits,
            &self.input_near_clips,
            &self.input_clips,
        );
    }

    fn record_output(&self, sample: f32) {
        self.record_sample(
            sample,
            &self.output_sum_squares,
            &self.output_count,
            &self.output_peak_bits,
            &self.output_near_clips,
            &self.output_clips,
        );
    }

    fn record_input_overrun(&self) {
        self.input_overruns.fetch_add(1, Ordering::Relaxed);
    }

    fn record_output_underrun(&self) {
        self.output_underruns.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot_and_reset(&self) -> MonitorSnapshot {
        MonitorSnapshot {
            input_sum_squares: self.input_sum_squares.swap(0, Ordering::Relaxed),
            output_sum_squares: self.output_sum_squares.swap(0, Ordering::Relaxed),
            input_count: self.input_count.swap(0, Ordering::Relaxed),
            output_count: self.output_count.swap(0, Ordering::Relaxed),
            input_peak: f32::from_bits(self.input_peak_bits.swap(0, Ordering::Relaxed)),
            output_peak: f32::from_bits(self.output_peak_bits.swap(0, Ordering::Relaxed)),
            input_near_clips: self.input_near_clips.swap(0, Ordering::Relaxed),
            output_near_clips: self.output_near_clips.swap(0, Ordering::Relaxed),
            input_clips: self.input_clips.swap(0, Ordering::Relaxed),
            output_clips: self.output_clips.swap(0, Ordering::Relaxed),
            input_overruns: self.input_overruns.swap(0, Ordering::Relaxed),
            output_underruns: self.output_underruns.swap(0, Ordering::Relaxed),
        }
    }

    fn record_sample(
        &self,
        sample: f32,
        sum_squares: &AtomicU64,
        count: &AtomicU64,
        peak_bits: &AtomicU32,
        near_clips: &AtomicU64,
        clips: &AtomicU64,
    ) {
        let magnitude = sample.abs();
        let square = (magnitude as f64 * magnitude as f64 * RMS_SCALE).round() as u64;
        sum_squares.fetch_add(square, Ordering::Relaxed);
        count.fetch_add(1, Ordering::Relaxed);
        update_peak(peak_bits, magnitude);
        if magnitude >= NEAR_CLIP_LEVEL {
            near_clips.fetch_add(1, Ordering::Relaxed);
        }
        if magnitude >= CLIP_LEVEL {
            clips.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn update_peak(peak_bits: &AtomicU32, magnitude: f32) {
    let magnitude_bits = magnitude.to_bits();
    let mut current = peak_bits.load(Ordering::Relaxed);
    while magnitude_bits > current {
        match peak_bits.compare_exchange_weak(
            current,
            magnitude_bits,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(value) => current = value,
        }
    }
}

fn rms_from_scaled(sum_squares: u64, count: u64) -> f32 {
    if count == 0 {
        0.0
    } else {
        (sum_squares as f64 / RMS_SCALE / count as f64).sqrt() as f32
    }
}

fn dbfs(level: f32) -> f32 {
    if level > 0.0 {
        20.0 * level.log10()
    } else {
        f32::NEG_INFINITY
    }
}

struct Args {
    input_device: String,
    output_device: String,
    input_channel: usize,
    output_channels: Vec<usize>,
    sample_rate: u32,
    period_size: u32,
    controls: AmpControls,
    input_db: f32,
    input_gain: f32,
    output_db: f32,
    ir: bool,
    monitor: bool,
    dumble_model: bool,
}

fn main() -> Result<()> {
    let host = cpal::default_host();
    let args = parse_args(&host)?;
    let input_device = find_device(host.input_devices()?, &args.input_device, "input")?;
    let output_device = find_device(host.output_devices()?, &args.output_device, "output")?;

    let input_range = select_config(
        input_device.supported_input_configs()?,
        args.sample_rate,
        args.period_size,
        "input",
    )?;
    let output_range = select_config(
        output_device.supported_output_configs()?,
        args.sample_rate,
        args.period_size,
        "output",
    )?;
    let input_channels = input_range.channels() as usize;
    let output_channels = output_range.channels() as usize;

    if args.input_channel >= input_channels {
        bail!(
            "input channel {} is unavailable; '{}' exposes {} input channels",
            args.input_channel + 1,
            args.input_device,
            input_channels
        );
    }
    if let Some(channel) = args
        .output_channels
        .iter()
        .find(|&&ch| ch >= output_channels)
    {
        bail!(
            "output channel {} is unavailable; '{}' exposes {} output channels",
            channel + 1,
            args.output_device,
            output_channels
        );
    }

    let input_config = stream_config(&input_range, args.sample_rate, args.period_size);
    let output_config = stream_config(&output_range, args.sample_rate, args.period_size);
    let (mut producer, mut consumer) = RingBuffer::<f32>::new(args.period_size as usize * 8);
    let input_channel = args.input_channel;

    let monitoring = Arc::new(MonitorStats::default());
    let monitoring_input = monitoring.clone();
    let input_stream = input_device.build_input_stream(
        &input_config,
        move |data: &[f32], _| {
            for frame in data.chunks_exact(input_channels) {
                let sample = frame[input_channel];
                if args.monitor {
                    monitoring_input.record_input(sample);
                }
                if producer.push(sample).is_err() && args.monitor {
                    monitoring_input.record_input_overrun();
                }
            }
        },
        |error| eprintln!("input stream error: {error}"),
        None,
    )?;

    let controls = args.controls;
    let input_gain = args.input_gain;
    let mut amp = if args.dumble_model {
        VoxAmp::with_model(args.sample_rate as f32, "dumble")
    } else {
        VoxAmp::new(args.sample_rate as f32)
    };
    let mut speaker = args
        .ir
        .then(|| SpeakerStage::from_embedded_ir(args.sample_rate))
        .transpose()?;
    let ir_enabled = speaker.is_some();
    let selected_outputs = args.output_channels;
    let monitoring_output = monitoring.clone();
    let output_stream = output_device.build_output_stream(
        &output_config,
        move |data: &mut [f32], _| {
            for frame in data.chunks_exact_mut(output_channels) {
                let input = match consumer.pop() {
                    Ok(sample) => sample,
                    Err(_) => {
                        if args.monitor {
                            monitoring_output.record_output_underrun();
                        }
                        0.0
                    }
                } * input_gain;
                let amp_output = amp.process(input, controls);
                let output = speaker
                    .as_mut()
                    .map_or(amp_output, |speaker| speaker.process(amp_output, true));
                if args.monitor {
                    monitoring_output.record_output(output);
                }
                frame.fill(0.0);
                for &channel in &selected_outputs {
                    frame[channel] = output;
                }
            }
        },
        |error| eprintln!("output stream error: {error}"),
        None,
    )?;

    output_stream.play()?;
    input_stream.play()?;
    if args.monitor {
        let monitor = monitoring.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            let stats = monitor.snapshot_and_reset();
            let in_rms = rms_from_scaled(stats.input_sum_squares, stats.input_count);
            let out_rms = rms_from_scaled(stats.output_sum_squares, stats.output_count);
            eprintln!(
                "MON input rms {:.5} ({:+.1} dBFS) peak {:.5} ({:+.1} dBFS) near/clip {}/{} | output rms {:.5} ({:+.1} dBFS) peak {:.5} ({:+.1} dBFS) near/clip {}/{} | xrun in/out {}/{}",
                in_rms,
                dbfs(in_rms),
                stats.input_peak,
                dbfs(stats.input_peak),
                stats.input_near_clips,
                stats.input_clips,
                out_rms,
                dbfs(out_rms),
                stats.output_peak,
                dbfs(stats.output_peak),
                stats.output_near_clips,
                stats.output_clips,
                stats.input_overruns,
                stats.output_underruns
            );
        });
    }
    eprintln!(
        "VoxBox running: {} input channels, {} output channels, {} Hz, {} samples",
        input_channels, output_channels, args.sample_rate, args.period_size
    );
    eprintln!(
        "Speaker IR: {}",
        if ir_enabled { "enabled" } else { "disabled" }
    );
    eprintln!(
        "Controls: Input {:+.1} dB, Volume {:.1}, Bass {:.1}, Treble {:.1}, Cut {:.1}, Output {:+.1} dB",
        args.input_db,
        controls.volume * 10.0,
        controls.bass * 10.0,
        controls.treble * 10.0,
        controls.cut * 10.0,
        args.output_db
    );
    eprintln!("Press Ctrl-C to stop.");

    loop {
        thread::park();
    }
}

fn parse_args(host: &cpal::Host) -> Result<Args> {
    let mut input_device = None;
    let mut output_device = None;
    let mut input_channel = 1;
    let mut output_channels = "1,2".to_owned();
    let mut sample_rate = 48_000;
    let mut period_size = 256;
    let mut volume = 5.5;
    let mut bass = 5.0;
    let mut treble = 6.0;
    let mut cut = 3.5;
    let mut drive = 0.0;
    let mut presence = 0.0;
    let mut sag = 0.0;
    let mut input_db = 0.0;
    let mut output_db = -9.0;
    let mut ir = false;
    let mut monitor = false;
    let mut dumble_model = false;
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--device" => {
                let name = next_value(&mut args, "--device")?;
                input_device = Some(name.clone());
                output_device = Some(name);
            }
            "--input-device" => input_device = Some(next_value(&mut args, "--input-device")?),
            "--output-device" => output_device = Some(next_value(&mut args, "--output-device")?),
            "--input-channel" => {
                input_channel = next_value(&mut args, "--input-channel")?.parse()?
            }
            "--output-channels" => output_channels = next_value(&mut args, "--output-channels")?,
            "--sample-rate" => sample_rate = next_value(&mut args, "--sample-rate")?.parse()?,
            "--period-size" => period_size = next_value(&mut args, "--period-size")?.parse()?,
            "--volume" | "--gain" => volume = parse_pot(&mut args, "--volume")?,
            "--bass" => bass = parse_pot(&mut args, "--bass")?,
            "--treble" | "--tone" => treble = parse_pot(&mut args, "--treble")?,
            "--cut" => cut = parse_pot(&mut args, "--cut")?,
            "--input-db" => input_db = next_value(&mut args, "--input-db")?.parse()?,
            "--output-db" => output_db = next_value(&mut args, "--output-db")?.parse()?,
            "--ir" => ir = true,
            "--preset" => {
                let name = next_value(&mut args, "--preset")?.to_lowercase();
                match name.as_str() {
                    "dumble" | "dumble-clean" => {
                        // Dumble Overdrive Special - clean
                        volume = 6.5;
                        bass = 5.5;
                        treble = 6.0;
                        cut = 3.8;
                        output_db = -12.0;
                        drive = 2.0;
                        presence = 1.0;
                        sag = 0.2;
                        dumble_model = true;
                    }
                    "dumble-crunch" => {
                        volume = 8.0;
                        bass = 5.2;
                        treble = 6.0;
                        cut = 4.4;
                        output_db = -14.0;
                        drive = 4.5;
                        presence = 1.5;
                        sag = 0.3;
                        dumble_model = true;
                    }
                    "dumble-driven" => {
                        volume = 9.5;
                        bass = 5.0;
                        treble = 5.8;
                        cut = 5.0;
                        output_db = -16.0;
                        drive = 7.5;
                        presence = 2.0;
                        sag = 0.45;
                        dumble_model = true;
                    }
                    _ => bail!("unknown preset '{name}'"),
                }
            }
            "--drive" => drive = parse_pot(&mut args, "--drive")?,
            "--presence" => presence = parse_pot(&mut args, "--presence")?,
            "--sag" => sag = parse_pot(&mut args, "--sag")?,
            "--monitor" => monitor = true,
            "--list-devices" => {
                print_devices(host)?;
                std::process::exit(0);
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ => bail!("unknown argument '{arg}'; use --help"),
        }
    }

    if input_channel == 0 {
        bail!("--input-channel is one-based and must be at least 1");
    }
    if sample_rate == 0 {
        bail!("--sample-rate must be greater than zero");
    }
    if period_size == 0 {
        bail!("--period-size must be greater than zero");
    }
    if !(-60.0..=6.0).contains(&output_db) {
        bail!("--output-db must be between -60 and +6");
    }
    if !(-60.0..=24.0).contains(&input_db) {
        bail!("--input-db must be between -60 and +24");
    }
    let output_channels = output_channels
        .split(',')
        .map(|value| value.trim().parse::<usize>())
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if output_channels.is_empty() || output_channels.contains(&0) {
        bail!("--output-channels must contain one-based channel numbers");
    }

    Ok(Args {
        input_device: input_device.context("missing --device or --input-device")?,
        output_device: output_device.context("missing --device or --output-device")?,
        input_channel: input_channel - 1,
        output_channels: output_channels.into_iter().map(|ch| ch - 1).collect(),
        sample_rate,
        period_size,
        controls: AmpControls {
            volume: volume / 10.0,
            bass: bass / 10.0,
            treble: treble / 10.0,
            cut: cut / 10.0,
            output: 10.0_f32.powf(output_db / 20.0),
            drive: drive / 10.0,
            presence: presence / 10.0,
            sag: sag / 10.0,
        },
        input_db,
        input_gain: 10.0_f32.powf(input_db / 20.0),
        output_db,
        ir,
        monitor,
        dumble_model,
    })
}

fn parse_pot(args: &mut impl Iterator<Item = String>, option: &str) -> Result<f32> {
    let value = next_value(args, option)?.parse::<f32>()?;
    if !(0.0..=10.0).contains(&value) {
        bail!("{option} must be between 0 and 10");
    }
    Ok(value)
}

fn next_value(args: &mut impl Iterator<Item = String>, option: &str) -> Result<String> {
    args.next()
        .with_context(|| format!("missing value for {option}"))
}

fn find_device(
    devices: impl Iterator<Item = Device>,
    wanted: &str,
    direction: &str,
) -> Result<Device> {
    devices
        .filter_map(|device| device.name().ok().map(|name| (device, name)))
        .find(|(_, name)| name == wanted)
        .map(|(device, _)| device)
        .with_context(|| {
            format!("could not find {direction} device '{wanted}'; use --list-devices")
        })
}

fn select_config(
    configs: impl Iterator<Item = SupportedStreamConfigRange>,
    sample_rate: u32,
    period_size: u32,
    direction: &str,
) -> Result<SupportedStreamConfigRange> {
    let rate = SampleRate(sample_rate);
    configs
        .filter(|config| config.sample_format() == SampleFormat::F32)
        .find(|config| {
            (config.min_sample_rate()..=config.max_sample_rate()).contains(&rate)
                && match config.buffer_size() {
                    cpal::SupportedBufferSize::Range { min, max } => {
                        (*min..=*max).contains(&period_size)
                    }
                    cpal::SupportedBufferSize::Unknown => true,
                }
        })
        .with_context(|| {
            format!(
                "no f32 {direction} configuration supports {sample_rate} Hz / {period_size} samples"
            )
        })
}

fn stream_config(
    range: &SupportedStreamConfigRange,
    sample_rate: u32,
    period_size: u32,
) -> StreamConfig {
    StreamConfig {
        channels: range.channels(),
        sample_rate: SampleRate(sample_rate),
        buffer_size: BufferSize::Fixed(period_size),
    }
}

fn print_devices(host: &cpal::Host) -> Result<()> {
    eprintln!("Input devices:");
    for device in host.input_devices()? {
        eprintln!("  {}", device.name()?);
    }
    eprintln!("Output devices:");
    for device in host.output_devices()? {
        eprintln!("  {}", device.name()?);
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Usage: voxbox-standalone --device NAME [OPTIONS]\n\
         \n\
         Options:\n\
         \x20 --device NAME             Use the same input and output device\n\
         \x20 --input-device NAME       Input device name\n\
         \x20 --output-device NAME      Output device name\n\
         \x20 --input-channel N         One-based guitar input [default: 1]\n\
         \x20 --output-channels N,N     One-based monitor outputs [default: 1,2]\n\
         \x20 --sample-rate HZ          Sample rate [default: 48000]\n\
         \x20 --period-size SAMPLES     Buffer size [default: 256]\n\
         \x20 --volume N                Top Boost volume, 0-10 [default: 5.5]\n\
         \x20 --bass N                  Top Boost bass, 0-10 [default: 5.0]\n\
         \x20 --treble N                Top Boost treble, 0-10 [default: 6.0]\n\
         \x20 --cut N                   Power amp Cut, 0-10 [default: 3.5]\n\
         \x20 --input-db DB             Interface input calibration [default: 0]\n\
         \x20 --output-db DB            Safety output trim [default: -9]\n\
         \x20 --monitor                 Print input/output dBFS peaks, clip counts, and xruns\n\
         \x20 --ir                      Enable the embedded 200 ms speaker IR\n\
         \x20 --list-devices            List CoreAudio devices"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pot_values_use_real_amp_scale() {
        let mut valid = ["7.5".to_owned()].into_iter();
        assert_eq!(parse_pot(&mut valid, "--volume").unwrap(), 7.5);

        let mut too_high = ["10.1".to_owned()].into_iter();
        assert!(parse_pot(&mut too_high, "--volume").is_err());

        let mut negative = ["-0.1".to_owned()].into_iter();
        assert!(parse_pot(&mut negative, "--cut").is_err());
    }
}
