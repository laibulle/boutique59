from __future__ import annotations

import json
from dataclasses import asdict
from datetime import UTC, datetime
from pathlib import Path

from greybound_lab.audio import read_wav_mono
from greybound_lab.metrics import (
    align_by_latency,
    band_residual_metrics,
    compare_signals,
    linear_to_db,
    optimal_gain,
)
from greybound_lab.render import git_revision, relative_or_absolute
from greybound_lab.segments import SegmentSpec
from greybound_lab.spice import FIXTURES, klon_centaur_metrics, parse_wrdata


def write_minotaur_klon_triage(
    *,
    repo_root: Path,
    spice_data: Path,
    candidate_wav: Path,
    reference_wav: Path,
    report: Path,
    metadata: Path | None = None,
    sweep_report: Path | None = None,
    segments: list[SegmentSpec] | None = None,
    max_lag_ms: float = 100.0,
) -> None:
    fixture = FIXTURES["klon-centaur"]
    trace = parse_wrdata(spice_data, fixture.signals)
    spice = klon_centaur_metrics(trace)

    candidate = read_wav_mono(candidate_wav)
    reference = read_wav_mono(reference_wav)
    if candidate.sample_rate != reference.sample_rate:
        raise ValueError(
            f"sample-rate mismatch: candidate={candidate.sample_rate} Hz, "
            f"reference={reference.sample_rate} Hz"
        )

    metrics = compare_signals(
        candidate.samples,
        reference.samples,
        candidate.sample_rate,
        max_lag_ms=max_lag_ms,
        segments=segments,
    )
    aligned_candidate, aligned_reference = align_by_latency(
        candidate.samples,
        reference.samples,
        metrics.latency_samples,
    )
    corrected_candidate = aligned_candidate * optimal_gain(aligned_candidate, aligned_reference)
    bands = band_residual_metrics(corrected_candidate, aligned_reference, candidate.sample_rate)

    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(
        _triage_markdown(
            repo_root=repo_root,
            spice_data=spice_data,
            candidate_wav=candidate_wav,
            reference_wav=reference_wav,
            sweep_report=sweep_report,
            report=report,
            metrics=metrics,
            spice=spice,
            bands=bands,
        ),
        encoding="utf-8",
    )

    if metadata is not None:
        metadata.parent.mkdir(parents=True, exist_ok=True)
        metadata.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "created_at": datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
                    "generator": "greybound-lab minotaur-klon-triage",
                    "git_revision": git_revision(repo_root),
                    "inputs": {
                        "spice_data": relative_or_absolute(spice_data, repo_root),
                        "candidate_wav": relative_or_absolute(candidate_wav, repo_root),
                        "reference_wav": relative_or_absolute(reference_wav, repo_root),
                        "sweep_report": relative_or_absolute(sweep_report, repo_root) if sweep_report else None,
                    },
                    "metrics": {
                        "sample_rate_hz": metrics.sample_rate_hz,
                        "latency_samples": metrics.latency_samples,
                        "latency_ms": metrics.latency_ms,
                        "gain_db": metrics.gain_db,
                        "null_relative_db": metrics.null_relative_db,
                        "log_spectral_distance_db": metrics.log_spectral_distance_db,
                        "envelope_error_db": metrics.envelope_error_db,
                        "candidate_crest_db": metrics.candidate.crest_db,
                        "reference_crest_db": metrics.reference.crest_db,
                        "band_residual": asdict(bands),
                    },
                    "spice": asdict(spice),
                },
                indent=2,
                sort_keys=True,
            )
            + "\n",
            encoding="utf-8",
        )


def _triage_markdown(
    *,
    repo_root: Path,
    spice_data: Path,
    candidate_wav: Path,
    reference_wav: Path,
    sweep_report: Path | None,
    report: Path,
    metrics,
    spice,
    bands,
) -> str:
    decision = _decision(metrics)
    sweep_line = f"- Sweep report: `{relative_or_absolute(sweep_report, repo_root)}`\n" if sweep_report else ""
    return f"""# Minotaur/Klon Triage

## Inputs

- SPICE data: `{relative_or_absolute(spice_data, repo_root)}`
- Rust candidate: `{relative_or_absolute(candidate_wav, repo_root)}`
- NAM reference: `{relative_or_absolute(reference_wav, repo_root)}`
{sweep_line}- Report: `{relative_or_absolute(report, repo_root)}`

## NAM vs Rust

The comparison applies latency alignment and optimal makeup gain before scoring
the residual.

| Metric | Value |
| --- | ---: |
| Sample rate | {metrics.sample_rate_hz} Hz |
| Latency | {metrics.latency_samples} samples / {metrics.latency_ms:.3f} ms |
| Makeup gain candidate -> reference | {metrics.gain_db:.2f} dB |
| Null residual relative to NAM | {metrics.null_relative_db:.2f} dB |
| Log spectral distance | {metrics.log_spectral_distance_db:.2f} dB |
| Envelope error | {metrics.envelope_error_db:.2f} dB |
| Rust crest factor | {metrics.candidate.crest_db:.2f} dB |
| NAM crest factor | {metrics.reference.crest_db:.2f} dB |

## Residual Bands

These are residual RMS levels relative to the NAM reference after global gain
correction. Less negative means that band is still poorly matched.

| Band | Residual |
| --- | ---: |
| 40-250 Hz | {bands.low_db:.2f} dB |
| 250 Hz-1 kHz | {bands.low_mid_db:.2f} dB |
| 1-4 kHz | {bands.mid_db:.2f} dB |
| 4-8 kHz | {bands.presence_db:.2f} dB |
| 8-18 kHz | {bands.air_db:.2f} dB |

## SPICE Klon Anchor

The current ngspice fixture is a topology and node-level anchor for the Klon
audio path. Metrics are measured after the first 50 ms on the settled 1 kHz
transient, with DC removed around the bias point.

| Node/metric | Value |
| --- | ---: |
| Input RMS | {spice.input_rms_v * 1000.0:.3f} mV |
| Buffer RMS | {spice.buffer_rms_v * 1000.0:.3f} mV |
| Clean path RMS | {spice.clean_rms_v * 1000.0:.3f} mV |
| Drive stage RMS | {spice.drive_rms_v * 1000.0:.3f} mV |
| Clip node RMS | {spice.clip_rms_v * 1000.0:.3f} mV |
| Mix node RMS | {spice.mix_rms_v * 1000.0:.3f} mV |
| Tone node RMS | {spice.tone_rms_v * 1000.0:.3f} mV |
| Output RMS | {spice.output_rms_v * 1000.0:.3f} mV |
| Output gain | {spice.output_gain_db:.2f} dB |
| Clip peak | {spice.clip_peak_v * 1000.0:.3f} mV |
| Clip asymmetry | {spice.clip_asymmetry_v * 1000.0:.3f} mV |

## Triage

{decision}

## Neural Direction

Do not train a full-pedal black box from this state. The best next cell is a
targeted Minotaur drive/clip/tone block:

- input: buffered pedal input, normalized gain and treble controls, and a short causal history;
- target: SPICE clip/mix/tone behavior first, then NAM residual correction after the analytic topology is level matched;
- guardrail: keep the analytic clean path and output level outside the cell so the neural block cannot solve the wrong problem by moving volume only.

The current SPICE fixture is not yet a training dataset. It is one calibrated
transient used to validate topology and node scaling. The next concrete step is
to generate a Klon SPICE dataset across amplitude, frequency, gain, and treble,
then fit the small causal cell against `drive -> clip/mix/tone`.
"""


def _decision(metrics) -> str:
    level_aligned = abs(metrics.gain_db) <= 3.0
    spectral_gap = metrics.log_spectral_distance_db >= 8.0
    weak_null = metrics.null_relative_db >= -12.0
    envelope_gap = metrics.envelope_error_db >= -14.0

    if level_aligned and (spectral_gap or weak_null):
        return (
            "The current Rust Minotaur is level-aligned enough to continue, but the residual is still too high. "
            "That points to nonlinear transfer and tone-shaping error, not just output gain. "
            "The neural work should focus on the drive/clip/tone region."
        )
    if envelope_gap:
        return (
            "The envelope mismatch is still audible enough to treat dynamics as part of the target. "
            "Use a causal cell with short history rather than a memoryless waveshaper."
        )
    return (
        "The analytic implementation is close enough for listening tests. "
        "Further neural work should be narrow and used as an A/B improvement path, not a replacement for the full pedal."
    )
