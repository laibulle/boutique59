use anyhow::{bail, Context, Result};
use greybound::circuit::triode::{CommonCathodeParams, CommonCathodeStage, TriodeParams};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct DatasetManifest {
    sample_rate_hz: u32,
    stimuli: Vec<Stimulus>,
    splits: Splits,
}

#[derive(Debug, Deserialize)]
struct Stimulus {
    id: String,
    kind: String,
    path: String,
    parameters: Option<StimulusParameters>,
}

#[derive(Debug, Deserialize)]
struct StimulusParameters {
    settle_time_s: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct Splits {
    train: Vec<String>,
    validation: Vec<String>,
    test: Vec<String>,
}

#[derive(Clone, Copy)]
struct Sample {
    time_s: f32,
    input_v: f32,
    plate_v: f32,
}

struct Row {
    stimulus_id: String,
    split: String,
    kind: String,
    samples: usize,
    aligned_samples: usize,
    reference_rms_v: f64,
    rmse_v: f64,
    mae_v: f64,
    max_abs_error_v: f64,
    relative_rmse: f64,
    latency_samples: isize,
    latency_us: f64,
    optimal_gain: f64,
    optimal_gain_db: f64,
    aligned_rmse_v: f64,
    aligned_relative_rmse: f64,
    zero_baseline_rmse_v: f64,
}

fn main() -> Result<()> {
    let args = Args::parse()?;
    let manifest_text = fs::read_to_string(&args.manifest)
        .with_context(|| format!("failed to read manifest {}", args.manifest.display()))?;
    let manifest: DatasetManifest =
        json5::from_str(&manifest_text).context("failed to parse SPICE dataset manifest")?;
    let mut rows = Vec::new();
    for stimulus in &manifest.stimuli {
        let split = split_for(&manifest.splits, &stimulus.id);
        if args.split != "all" && args.split != split {
            continue;
        }
        let trace_path = resolve_path(&args.manifest, &stimulus.path);
        let samples = read_trace(&trace_path)
            .with_context(|| format!("failed to read SPICE trace {}", trace_path.display()))?;
        rows.push(evaluate_stimulus(
            &manifest,
            stimulus,
            split,
            &samples,
            args.stride,
        )?);
    }
    write_report(
        &args.report,
        &args.manifest,
        &rows,
        args.stride,
        &args.split,
    )?;
    println!("wrote {}", args.report.display());
    Ok(())
}

struct Args {
    manifest: PathBuf,
    report: PathBuf,
    stride: usize,
    split: String,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut manifest = None;
        let mut report = None;
        let mut stride = 16usize;
        let mut split = "all".to_string();
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--manifest" => manifest = args.next().map(PathBuf::from),
                "--report" => report = args.next().map(PathBuf::from),
                "--stride" => {
                    let value = args.next().context("--stride requires a value")?;
                    stride = value.parse().context("invalid --stride")?;
                }
                "--split" => split = args.next().context("--split requires a value")?,
                other => bail!("unknown argument '{}'", other),
            }
        }
        let manifest = manifest.context("--manifest is required")?;
        let report = report.context("--report is required")?;
        if stride == 0 {
            bail!("--stride must be greater than zero");
        }
        Ok(Self {
            manifest,
            report,
            stride,
            split,
        })
    }
}

fn evaluate_stimulus(
    manifest: &DatasetManifest,
    stimulus: &Stimulus,
    split: &str,
    samples: &[Sample],
    stride: usize,
) -> Result<Row> {
    let settle_time_s = stimulus
        .parameters
        .as_ref()
        .and_then(|parameters| parameters.settle_time_s)
        .unwrap_or(0.0);
    let mut stage = CommonCathodeStage::new(CommonCathodeParams {
        sample_rate: manifest.sample_rate_hz as f32,
        grid_leak_resistance: 1_000_000.0,
        input_coupling_capacitance: 22e-9,
        plate_resistance: 100_000.0,
        cathode_resistance: 1_500.0,
        cathode_bypass_capacitance: Some(25e-6),
        supply_resistance: 10_000.0,
        supply_capacitance: 22e-6,
        nominal_supply_voltage: 280.0,
        input_gain: 1.0,
        output_scale: 1.0,
        triode: TriodeParams::ECC83,
    });
    for _ in 0..(manifest.sample_rate_hz as usize / 20) {
        stage.process(0.0);
    }
    let mut reference = Vec::new();
    let mut candidate = Vec::new();
    for (index, sample) in samples.iter().enumerate() {
        stage.process(sample.input_v);
        let output = stage.operating_point().plate_voltage;
        if sample.time_s >= settle_time_s && index % stride == 0 {
            reference.push(sample.plate_v);
            candidate.push(output);
        }
    }
    if reference.is_empty() {
        bail!("stimulus '{}' has no samples after settling", stimulus.id);
    }
    remove_mean(&mut reference);
    remove_mean(&mut candidate);
    let errors: Vec<f64> = candidate
        .iter()
        .zip(reference.iter())
        .map(|(candidate, reference)| (*candidate as f64) - (*reference as f64))
        .collect();
    let reference_f64: Vec<f64> = reference.iter().map(|value| *value as f64).collect();
    let reference_rms = rms(&reference_f64);
    let rmse = rms(&errors);
    let mae = errors.iter().map(|value| value.abs()).sum::<f64>() / errors.len() as f64;
    let max_abs = errors.iter().map(|value| value.abs()).fold(0.0, f64::max);
    let alignment = best_gain_latency_alignment(
        &candidate
            .iter()
            .map(|value| *value as f64)
            .collect::<Vec<_>>(),
        &reference_f64,
        manifest.sample_rate_hz,
        stride,
    )?;
    Ok(Row {
        stimulus_id: stimulus.id.clone(),
        split: split.to_string(),
        kind: stimulus.kind.clone(),
        samples: reference.len(),
        aligned_samples: alignment.samples,
        reference_rms_v: reference_rms,
        rmse_v: rmse,
        mae_v: mae,
        max_abs_error_v: max_abs,
        relative_rmse: rmse / reference_rms.max(1.0e-12),
        latency_samples: alignment.latency_samples,
        latency_us: alignment.latency_us,
        optimal_gain: alignment.gain,
        optimal_gain_db: linear_to_db(alignment.gain.abs().max(1.0e-12)),
        aligned_rmse_v: alignment.rmse_v,
        aligned_relative_rmse: alignment.rmse_v / alignment.reference_rms_v.max(1.0e-12),
        zero_baseline_rmse_v: reference_rms,
    })
}

fn read_trace(path: &Path) -> Result<Vec<Sample>> {
    let text = fs::read_to_string(path)?;
    let mut samples = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let values = line
            .split_whitespace()
            .map(str::parse::<f32>)
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| format!("invalid number at {}:{}", path.display(), line_number + 1))?;
        if values.len() != 10 {
            bail!(
                "{}:{} has {} columns, expected 10",
                path.display(),
                line_number + 1,
                values.len()
            );
        }
        samples.push(Sample {
            time_s: values[0],
            input_v: values[1],
            plate_v: values[5],
        });
    }
    Ok(samples)
}

fn write_report(
    path: &Path,
    manifest_path: &Path,
    rows: &[Row],
    stride: usize,
    split: &str,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let aggregate = aggregate(rows);
    let table = rows
        .iter()
        .map(|row| {
            format!(
                "| `{}` | `{}` | `{}` | {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.2}% | {} | {:.1} | {:.4} | {:.2} | {:.3} | {:.2}% | {:.3} |",
                row.stimulus_id,
                row.split,
                row.kind,
                row.samples,
                row.aligned_samples,
                row.reference_rms_v * 1000.0,
                row.rmse_v * 1000.0,
                row.mae_v * 1000.0,
                row.max_abs_error_v * 1000.0,
                row.relative_rmse * 100.0,
                row.latency_samples,
                row.latency_us,
                row.optimal_gain,
                row.optimal_gain_db,
                row.aligned_rmse_v * 1000.0,
                row.aligned_relative_rmse * 100.0,
                row.zero_baseline_rmse_v * 1000.0
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        path,
        format!(
            r#"# Analytic Common-Cathode SPICE Evaluation

## Purpose

Evaluate Greybound's current Rust `CommonCathodeStage` analytic cell against the
SPICE dataset in physical units. This is the analytic baseline for neural-cell
experiments.

The stage is configured with the same component values as the SPICE fixture and
pre-settled before each stimulus. Its internal plate voltage is recentered per
stimulus before comparison with SPICE `plate_ac_v`.

## Inputs

- Dataset manifest: `{}`
- Split filter: `{}`
- Evaluation stride: `{}`

## Aggregate

| Metric | Value |
| --- | ---: |
| Stimuli evaluated | {} |
| Samples evaluated | {} |
| Weighted RMSE | {:.3} mV |
| Weighted MAE | {:.3} mV |
| Weighted relative RMSE | {:.2}% |
| Weighted aligned RMSE | {:.3} mV |
| Weighted aligned relative RMSE | {:.2}% |
| Zero baseline RMSE | {:.3} mV |

## Per-Stimulus Metrics

| Stimulus | Split | Kind | Samples | Aligned samples | Ref RMS mV | RMSE mV | MAE mV | Max abs mV | Rel RMSE | Best lag | Lag us | Gain | Gain dB | Aligned RMSE mV | Aligned Rel RMSE | Zero baseline RMSE mV |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
{}

## Interpretation

This report is the first reference point for deciding whether a neural cell is
actually better than the existing Rust analytic solver. A neural replacement
should improve held-out stimuli, not merely beat a zero baseline.

The aligned residual applies only a small integer-latency search and an optimal
linear gain. If it drops far below the raw residual, the mismatch is dominated
by timing or level calibration. If it remains close to the raw residual, the
remaining error is more likely model shape, nonlinear transfer, bias dynamics,
or discretization.

This alignment is diagnostic, not a physical latency measurement. Periodic
stimuli can produce phase-equivalent lags and negative gains, especially around
single-frequency sine tests.
"#,
            manifest_path.display(),
            split,
            stride,
            rows.len(),
            aggregate.samples,
            aggregate.rmse_v * 1000.0,
            aggregate.mae_v * 1000.0,
            aggregate.relative_rmse * 100.0,
            aggregate.aligned_rmse_v * 1000.0,
            aggregate.aligned_relative_rmse * 100.0,
            aggregate.zero_baseline_rmse_v * 1000.0,
            table
        ),
    )?;
    Ok(())
}

struct Aggregate {
    samples: usize,
    rmse_v: f64,
    mae_v: f64,
    relative_rmse: f64,
    aligned_rmse_v: f64,
    aligned_relative_rmse: f64,
    zero_baseline_rmse_v: f64,
}

fn aggregate(rows: &[Row]) -> Aggregate {
    let samples = rows.iter().map(|row| row.samples).sum::<usize>();
    if samples == 0 {
        return Aggregate {
            samples: 0,
            rmse_v: 0.0,
            mae_v: 0.0,
            relative_rmse: 0.0,
            aligned_rmse_v: 0.0,
            aligned_relative_rmse: 0.0,
            zero_baseline_rmse_v: 0.0,
        };
    }
    let weighted = |value: fn(&Row) -> f64| {
        rows.iter()
            .map(|row| value(row) * row.samples as f64)
            .sum::<f64>()
            / samples as f64
    };
    let rmse_v = weighted(|row| row.rmse_v.powi(2)).sqrt();
    let mae_v = weighted(|row| row.mae_v);
    let reference_rms_v = weighted(|row| row.reference_rms_v.powi(2)).sqrt();
    let aligned_rmse_v = weighted(|row| row.aligned_rmse_v.powi(2)).sqrt();
    let zero_baseline_rmse_v = weighted(|row| row.zero_baseline_rmse_v.powi(2)).sqrt();
    Aggregate {
        samples,
        rmse_v,
        mae_v,
        relative_rmse: rmse_v / reference_rms_v.max(1.0e-12),
        aligned_rmse_v,
        aligned_relative_rmse: aligned_rmse_v / reference_rms_v.max(1.0e-12),
        zero_baseline_rmse_v,
    }
}

fn resolve_path(manifest_path: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return path;
    }
    if path.exists() {
        return path;
    }
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(path)
}

fn split_for<'a>(splits: &'a Splits, stimulus_id: &str) -> &'a str {
    if splits.train.iter().any(|item| item == stimulus_id) {
        "train"
    } else if splits.validation.iter().any(|item| item == stimulus_id) {
        "validation"
    } else if splits.test.iter().any(|item| item == stimulus_id) {
        "test"
    } else {
        "unknown"
    }
}

fn remove_mean(values: &mut [f32]) {
    let mean = values.iter().map(|value| *value as f64).sum::<f64>() / values.len() as f64;
    for value in values {
        *value -= mean as f32;
    }
}

fn rms(values: &[f64]) -> f64 {
    (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt()
}

struct Alignment {
    samples: usize,
    latency_samples: isize,
    latency_us: f64,
    gain: f64,
    rmse_v: f64,
    reference_rms_v: f64,
}

fn best_gain_latency_alignment(
    candidate: &[f64],
    reference: &[f64],
    sample_rate_hz: u32,
    stride: usize,
) -> Result<Alignment> {
    if candidate.len() != reference.len() {
        bail!("candidate/reference length mismatch");
    }
    let max_lag = 64usize.min(candidate.len().saturating_sub(1) / 4);
    let mut best: Option<Alignment> = None;
    for lag in -(max_lag as isize)..=(max_lag as isize) {
        let (candidate_start, reference_start) = if lag >= 0 {
            (lag as usize, 0usize)
        } else {
            (0usize, (-lag) as usize)
        };
        let length = (candidate.len() - candidate_start).min(reference.len() - reference_start);
        if length < 8 {
            continue;
        }
        let candidate_slice = &candidate[candidate_start..candidate_start + length];
        let reference_slice = &reference[reference_start..reference_start + length];
        let gain = optimal_gain(candidate_slice, reference_slice);
        let residual = candidate_slice
            .iter()
            .zip(reference_slice.iter())
            .map(|(candidate, reference)| candidate * gain - reference)
            .collect::<Vec<_>>();
        let rmse_v = rms(&residual);
        let reference_rms_v = rms(reference_slice);
        let alignment = Alignment {
            samples: length,
            latency_samples: lag,
            latency_us: 1_000_000.0 * lag as f64 * stride as f64 / sample_rate_hz as f64,
            gain,
            rmse_v,
            reference_rms_v,
        };
        if best
            .as_ref()
            .map(|current| alignment.rmse_v < current.rmse_v)
            .unwrap_or(true)
        {
            best = Some(alignment);
        }
    }
    best.context("latency alignment produced no valid overlap")
}

fn optimal_gain(candidate: &[f64], reference: &[f64]) -> f64 {
    let denominator = candidate
        .iter()
        .map(|candidate| candidate * candidate)
        .sum::<f64>();
    if denominator <= 1.0e-18 {
        return 1.0;
    }
    candidate
        .iter()
        .zip(reference.iter())
        .map(|(candidate, reference)| candidate * reference)
        .sum::<f64>()
        / denominator
}

fn linear_to_db(value: f64) -> f64 {
    20.0 * value.max(1.0e-12).log10()
}
