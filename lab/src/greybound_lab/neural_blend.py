from __future__ import annotations

import json
from dataclasses import asdict, dataclass
from pathlib import Path

import numpy as np
from scipy.io import wavfile

from greybound_lab.audio import read_wav_mono
from greybound_lab.integrated_neural import NamMatchScore, nam_match_score
from greybound_lab.metrics import ComparisonMetrics, compare_signals
from greybound_lab.segments import SegmentSpec


@dataclass(frozen=True)
class BlendPoint:
    alpha: float
    wav_path: Path
    metrics: ComparisonMetrics
    score: NamMatchScore
    program_metrics: ComparisonMetrics | None
    program_score: NamMatchScore | None


def run_neural_blend_sweep(
    *,
    analytic_wav: Path,
    replace_wav: Path,
    reference_wav: Path,
    output_dir: Path,
    report: Path,
    metadata: Path,
    alphas: list[float],
    segments: list[SegmentSpec] | None = None,
    max_lag_ms: float = 100.0,
) -> list[BlendPoint]:
    analytic = read_wav_mono(analytic_wav)
    replace = read_wav_mono(replace_wav)
    reference = read_wav_mono(reference_wav)
    if analytic.sample_rate != replace.sample_rate or analytic.sample_rate != reference.sample_rate:
        raise ValueError("blend sweep WAV sample-rate mismatch")
    length = min(analytic.samples.shape[0], replace.samples.shape[0], reference.samples.shape[0])
    analytic_samples = analytic.samples[:length].astype(np.float64)
    replace_samples = replace.samples[:length].astype(np.float64)
    reference_samples = reference.samples[:length].astype(np.float64)

    output_dir.mkdir(parents=True, exist_ok=True)
    program_start_s = _program_material_start_s(segments)
    points = []
    for alpha in alphas:
        alpha = float(alpha)
        if alpha < 0.0 or alpha > 1.0:
            raise ValueError(f"blend alpha must be between 0 and 1: {alpha}")
        blended = (1.0 - alpha) * analytic_samples + alpha * replace_samples
        wav_path = output_dir / f"blend-{_format_alpha(alpha)}.wav"
        wavfile.write(wav_path, analytic.sample_rate, blended.astype(np.float32))
        metrics = compare_signals(
            blended,
            reference_samples,
            analytic.sample_rate,
            max_lag_ms=max_lag_ms,
            segments=segments,
        )
        program_metrics = None
        program_score = None
        if program_start_s is not None and program_start_s > 0.0:
            start = int(round(program_start_s * analytic.sample_rate))
            program_metrics = compare_signals(
                blended[start:],
                reference_samples[start:],
                analytic.sample_rate,
                max_lag_ms=max_lag_ms,
            )
            program_score = nam_match_score(program_metrics)
        points.append(
            BlendPoint(
                alpha=alpha,
                wav_path=wav_path,
                metrics=metrics,
                score=nam_match_score(metrics),
                program_metrics=program_metrics,
                program_score=program_score,
            )
        )
    write_blend_report(report, points, analytic_wav, replace_wav, reference_wav, program_start_s)
    write_blend_metadata(metadata, points, analytic_wav, replace_wav, reference_wav, program_start_s)
    return points


def write_blend_report(
    path: Path,
    points: list[BlendPoint],
    analytic_wav: Path,
    replace_wav: Path,
    reference_wav: Path,
    program_start_s: float | None,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    best = min(points, key=lambda point: point.score.total)
    best_program = min(
        (point for point in points if point.program_score is not None),
        key=lambda point: point.program_score.total if point.program_score else float("inf"),
        default=None,
    )
    rows = "\n".join(
        "| {:.3f} | {:.4f} | {:.2f} | {:.2f} | {:.2f} | {:.3f} | {:.4f} | `{}` |".format(
            point.alpha,
            point.score.total,
            point.metrics.log_spectral_distance_db,
            point.metrics.null_relative_db,
            point.metrics.envelope_error_db,
            point.metrics.gain_db,
            point.program_score.total if point.program_score else float("nan"),
            point.wav_path,
        )
        for point in points
    )
    program_note = (
        f"Program-material score excludes explicit preroll before `{program_start_s:.3f} s`."
        if program_start_s is not None
        else "Program-material score was not computed because no preroll-aware segments were provided."
    )
    best_program_line = (
        f"- Best program-material alpha: `{best_program.alpha:.3f}` score `{best_program.program_score.total:.4f}`."
        if best_program and best_program.program_score
        else "- Best program-material alpha: n/a."
    )
    path.write_text(
        f"""# Neural Blend Sweep

## Inputs

- Analytic WAV: `{analytic_wav}`
- Neural replace WAV: `{replace_wav}`
- NAM reference WAV: `{reference_wav}`

## Summary

- Best global alpha: `{best.alpha:.3f}` score `{best.score.total:.4f}`.
{best_program_line}
- {program_note}

`alpha=0` is pure analytic. `alpha=1` is full neural replace. Lower score is better.

## Points

| Alpha | Score | LSD dB | Null rel dB | Envelope dB | Gain dB | Program score | WAV |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
{rows}
""",
        encoding="utf-8",
    )


def write_blend_metadata(
    path: Path,
    points: list[BlendPoint],
    analytic_wav: Path,
    replace_wav: Path,
    reference_wav: Path,
    program_start_s: float | None,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema_version": 1,
        "analytic_wav": str(analytic_wav),
        "replace_wav": str(replace_wav),
        "reference_wav": str(reference_wav),
        "program_start_s": program_start_s,
        "points": [
            {
                "alpha": point.alpha,
                "wav_path": str(point.wav_path),
                "score": asdict(point.score),
                "program_score": asdict(point.program_score) if point.program_score else None,
                "metrics": {
                    "gain_db": point.metrics.gain_db,
                    "null_relative_db": point.metrics.null_relative_db,
                    "log_spectral_distance_db": point.metrics.log_spectral_distance_db,
                    "envelope_error_db": point.metrics.envelope_error_db,
                },
                "program_metrics": {
                    "gain_db": point.program_metrics.gain_db,
                    "null_relative_db": point.program_metrics.null_relative_db,
                    "log_spectral_distance_db": point.program_metrics.log_spectral_distance_db,
                    "envelope_error_db": point.program_metrics.envelope_error_db,
                }
                if point.program_metrics
                else None,
            }
            for point in points
        ],
    }
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def parse_alpha_csv(value: str) -> list[float]:
    try:
        return [float(part.strip()) for part in value.split(",") if part.strip()]
    except ValueError as exc:
        raise ValueError(f"expected comma-separated blend alphas: {value}") from exc


def _format_alpha(alpha: float) -> str:
    return f"{alpha:.3f}".replace(".", "p")


def _program_material_start_s(segments: list[SegmentSpec] | None) -> float | None:
    if not segments:
        return None
    starts = [segment.start_s for segment in segments if segment.kind.lower() != "preroll"]
    return min(starts) if starts else None
