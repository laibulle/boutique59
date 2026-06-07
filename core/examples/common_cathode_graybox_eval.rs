use anyhow::{bail, Context, Result};
use greybound::neural_cell::CommonCathodeGrayboxStateCell;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct DatasetManifest {
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
    reference_rms_v: f64,
    rmse_v: f64,
    mae_v: f64,
    max_abs_error_v: f64,
    relative_rmse: f64,
}

struct Args {
    manifest: PathBuf,
    config: PathBuf,
    report: PathBuf,
    stride: usize,
    split: String,
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
            stimulus,
            split,
            &samples,
            &args.config,
            args.stride,
        )?);
    }
    write_report(
        &args.report,
        &args.manifest,
        &args.config,
        &rows,
        args.stride,
        &args.split,
    )?;
    println!("wrote {}", args.report.display());
    Ok(())
}

impl Args {
    fn parse() -> Result<Self> {
        let mut manifest = None;
        let mut config = None;
        let mut report = None;
        let mut stride = 16usize;
        let mut split = "all".to_string();
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--manifest" => manifest = args.next().map(PathBuf::from),
                "--config" => config = args.next().map(PathBuf::from),
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
        let config = config.context("--config is required")?;
        let report = report.context("--report is required")?;
        if stride == 0 {
            bail!("--stride must be greater than zero");
        }
        Ok(Self {
            manifest,
            config,
            report,
            stride,
            split,
        })
    }
}

fn evaluate_stimulus(
    stimulus: &Stimulus,
    split: &str,
    samples: &[Sample],
    config_path: &Path,
    stride: usize,
) -> Result<Row> {
    let settle_time_s = stimulus
        .parameters
        .as_ref()
        .and_then(|parameters| parameters.settle_time_s)
        .unwrap_or(0.0);
    let mut cell = CommonCathodeGrayboxStateCell::from_config_path(config_path)?;
    let plate_mean = samples
        .iter()
        .map(|sample| sample.plate_v as f64)
        .sum::<f64>()
        / samples.len().max(1) as f64;
    let mut reference = Vec::new();
    let mut candidate = Vec::new();
    let mut settled_index = 0usize;
    for sample in samples {
        if sample.time_s >= settle_time_s {
            if settled_index % stride == 0 {
                reference.push((sample.plate_v as f64 - plate_mean) as f32);
                candidate.push(cell.process_sample(sample.input_v));
            }
            settled_index += 1;
        }
    }
    if reference.is_empty() {
        bail!("stimulus '{}' has no samples after settling", stimulus.id);
    }
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
    Ok(Row {
        stimulus_id: stimulus.id.clone(),
        split: split.to_string(),
        kind: stimulus.kind.clone(),
        samples: reference.len(),
        reference_rms_v: reference_rms,
        rmse_v: rmse,
        mae_v: mae,
        max_abs_error_v: max_abs,
        relative_rmse: rmse / reference_rms.max(1.0e-12),
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
    config_path: &Path,
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
                "| `{}` | `{}` | `{}` | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.2}% |",
                row.stimulus_id,
                row.split,
                row.kind,
                row.samples,
                row.reference_rms_v * 1000.0,
                row.rmse_v * 1000.0,
                row.mae_v * 1000.0,
                row.max_abs_error_v * 1000.0,
                row.relative_rmse * 100.0,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        path,
        format!(
            r#"# Rust Common-Cathode Gray-Box Evaluation

## Purpose

Evaluate the experimental Rust `CommonCathodeGrayboxStateCell` against the
common-cathode SPICE dataset using a fitted gray-box parameter JSON.

This is still an R&D probe, not an accepted runtime model.

## Inputs

- Dataset manifest: `{}`
- Gray-box config: `{}`
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

## Per-Stimulus Metrics

| Stimulus | Split | Kind | Samples | Ref RMS mV | RMSE mV | MAE mV | Max abs mV | Rel RMSE |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
{}
"#,
            manifest_path.display(),
            config_path.display(),
            split,
            stride,
            rows.len(),
            aggregate.samples,
            aggregate.rmse_v * 1000.0,
            aggregate.mae_v * 1000.0,
            aggregate.relative_rmse * 100.0,
            table,
        ),
    )?;
    Ok(())
}

struct Aggregate {
    samples: usize,
    rmse_v: f64,
    mae_v: f64,
    relative_rmse: f64,
}

fn aggregate(rows: &[Row]) -> Aggregate {
    let samples = rows.iter().map(|row| row.samples).sum::<usize>();
    if samples == 0 {
        return Aggregate {
            samples: 0,
            rmse_v: 0.0,
            mae_v: 0.0,
            relative_rmse: 0.0,
        };
    }
    let rmse = (rows
        .iter()
        .map(|row| row.rmse_v * row.rmse_v * row.samples as f64)
        .sum::<f64>()
        / samples as f64)
        .sqrt();
    let mae = rows
        .iter()
        .map(|row| row.mae_v * row.samples as f64)
        .sum::<f64>()
        / samples as f64;
    let reference_rms = (rows
        .iter()
        .map(|row| row.reference_rms_v * row.reference_rms_v * row.samples as f64)
        .sum::<f64>()
        / samples as f64)
        .sqrt();
    Aggregate {
        samples,
        rmse_v: rmse,
        mae_v: mae,
        relative_rmse: rmse / reference_rms.max(1.0e-12),
    }
}

fn split_for<'a>(splits: &'a Splits, stimulus_id: &str) -> &'a str {
    if splits.train.iter().any(|id| id == stimulus_id) {
        "train"
    } else if splits.validation.iter().any(|id| id == stimulus_id) {
        "validation"
    } else if splits.test.iter().any(|id| id == stimulus_id) {
        "test"
    } else {
        "unknown"
    }
}

fn resolve_path(base_path: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        let relative_to_manifest = base_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&path);
        if relative_to_manifest.exists() {
            relative_to_manifest
        } else {
            path
        }
    }
}

fn rms(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt()
    }
}
