from __future__ import annotations

from dataclasses import asdict, dataclass
import json
from pathlib import Path
from typing import Iterable

import numpy as np

from greybound_lab.metrics import ComparisonMetrics


@dataclass(frozen=True)
class EvaluationGate:
    name: str
    status: str
    value: float
    warning: float
    severe: float
    unit: str
    note: str


@dataclass(frozen=True)
class EvaluationResult:
    profile: str
    verdict: str
    gates: tuple[EvaluationGate, ...]
    near_clip_count: int
    hard_clip_count: int


def evaluate_metrics(
    metrics: ComparisonMetrics,
    candidate_samples: np.ndarray,
    *,
    profile: str = "amp-tone",
) -> EvaluationResult:
    candidate = np.asarray(candidate_samples, dtype=np.float64)
    near_clip_count = int(np.count_nonzero(np.abs(candidate) >= 0.95))
    hard_clip_count = int(np.count_nonzero(np.abs(candidate) >= 0.999))

    gates = [
        _upper_gate("candidate_peak", metrics.candidate.peak_dbfs, -1.0, -0.1, "dBFS", "raw candidate peak level"),
        _upper_gate("hard_clip_samples", float(hard_clip_count), 0.0, 0.0, "samples", "samples at or above 0.999 FS"),
        _upper_gate("near_clip_samples", float(near_clip_count), 32.0, 512.0, "samples", "samples at or above 0.95 FS"),
        _upper_gate("candidate_dc_mean", metrics.candidate.mean_dbfs, -60.0, -40.0, "dBFS", "raw candidate DC offset"),
        _upper_gate("aligned_dc_delta", metrics.dc_offset_delta_db, -70.0, -50.0, "dBFS", "candidate/reference DC mismatch"),
        _upper_gate("gain_correction_abs", abs(metrics.gain_db), 6.0, 12.0, "dB", "alignment gain correction magnitude"),
    ]
    gates.extend(_profile_gates(metrics, profile))

    verdict = _worst_status(gate.status for gate in gates)
    return EvaluationResult(
        profile=profile,
        verdict=verdict,
        gates=tuple(gates),
        near_clip_count=near_clip_count,
        hard_clip_count=hard_clip_count,
    )


def write_evaluation_report(
    path: Path,
    *,
    candidate_path: Path,
    reference_path: Path,
    metrics: ComparisonMetrics,
    result: EvaluationResult,
    metadata_path: Path | None = None,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    metadata_line = str(metadata_path) if metadata_path else "not provided"
    path.write_text(
        _render_evaluation_markdown(
            candidate_path=candidate_path,
            reference_path=reference_path,
            metadata_line=metadata_line,
            metrics=metrics,
            result=result,
        ),
        encoding="utf-8",
    )


def write_evaluation_json(path: Path, *, metrics: ComparisonMetrics, result: EvaluationResult) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {
                "profile": result.profile,
                "verdict": result.verdict,
                "metrics": {
                    "latency_samples": metrics.latency_samples,
                    "latency_ms": metrics.latency_ms,
                    "gain_db": metrics.gain_db,
                    "candidate_rms_dbfs": metrics.candidate.rms_dbfs,
                    "candidate_peak_dbfs": metrics.candidate.peak_dbfs,
                    "candidate_crest_db": metrics.candidate.crest_db,
                    "candidate_mean_dbfs": metrics.candidate.mean_dbfs,
                    "null_relative_db": metrics.null_relative_db,
                    "log_spectral_distance_db": metrics.log_spectral_distance_db,
                    "weighted_log_spectral_distance_db": metrics.weighted_log_spectral_distance_db,
                    "envelope_error_db": metrics.envelope_error_db,
                    "dc_offset_delta_db": metrics.dc_offset_delta_db,
                    "near_clip_count": result.near_clip_count,
                    "hard_clip_count": result.hard_clip_count,
                },
                "gates": [asdict(gate) for gate in result.gates],
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )


def _profile_gates(metrics: ComparisonMetrics, profile: str) -> list[EvaluationGate]:
    if profile == "regression":
        return [
            _upper_gate("null_relative", metrics.null_relative_db, -60.0, -40.0, "dB", "strict residual regression gate"),
            _upper_gate("weighted_lsd", metrics.weighted_log_spectral_distance_db, 2.0, 6.0, "dB", "weighted spectral regression gate"),
            _upper_gate("envelope_error", metrics.envelope_error_db, -40.0, -24.0, "dB", "strict envelope regression gate"),
        ]
    if profile == "clipper":
        gates = [
            _upper_gate("weighted_lsd", metrics.weighted_log_spectral_distance_db, 12.0, 20.0, "dB", "clipper spectral target gate"),
            _upper_gate("envelope_error", metrics.envelope_error_db, -8.0, -4.0, "dB", "clipper envelope target gate"),
        ]
        for segment in metrics.segments:
            if segment.harmonics is not None:
                gates.append(
                    _upper_gate(
                        f"{segment.name}.thd_delta_abs",
                        abs(segment.harmonics.thd_delta_db),
                        6.0,
                        12.0,
                        "dB",
                        "harmonic distortion shape mismatch",
                    )
                )
            if segment.imd is not None:
                gates.append(
                    _upper_gate(
                        f"{segment.name}.imd_delta_abs",
                        abs(segment.imd.imd_delta_db),
                        6.0,
                        12.0,
                        "dB",
                        "intermodulation shape mismatch",
                    )
                )
            if segment.aliasing is not None:
                gates.append(
                    _upper_gate(
                        f"{segment.name}.residual_high_band",
                        segment.aliasing.residual_high_band_dbfs,
                        -90.0,
                        -70.0,
                        "dBFS",
                        "high-band residual on aliasing stress segment",
                    )
                )
        return gates
    if profile == "amp-tone":
        return [
            _upper_gate("null_relative", metrics.null_relative_db, -6.0, -3.0, "dB", "full-rig residual target gate"),
            _upper_gate("log_spectral_distance", metrics.log_spectral_distance_db, 14.0, 22.0, "dB", "unweighted spectral target gate"),
            _upper_gate("weighted_lsd", metrics.weighted_log_spectral_distance_db, 10.0, 18.0, "dB", "guitar-band spectral target gate"),
            _upper_gate("envelope_error", metrics.envelope_error_db, -8.0, -4.0, "dB", "dynamic-envelope target gate"),
        ]
    raise ValueError(f"unsupported evaluation profile: {profile}")


def _upper_gate(name: str, value: float, warning: float, severe: float, unit: str, note: str) -> EvaluationGate:
    if warning == severe:
        status = "severe" if value > severe else "pass"
    elif severe > warning:
        status = "severe" if value >= severe else "warning" if value >= warning else "pass"
    else:
        status = "severe" if value >= severe else "warning" if value >= warning else "pass"
    return EvaluationGate(
        name=name,
        status=status,
        value=float(value),
        warning=float(warning),
        severe=float(severe),
        unit=unit,
        note=note,
    )


def _worst_status(statuses: Iterable[str]) -> str:
    rank = {"pass": 0, "warning": 1, "severe": 2}
    worst = "pass"
    for status in statuses:
        if rank[status] > rank[worst]:
            worst = status
    return worst


def _render_evaluation_markdown(
    *,
    candidate_path: Path,
    reference_path: Path,
    metadata_line: str,
    metrics: ComparisonMetrics,
    result: EvaluationResult,
) -> str:
    gate_lines = [
        "| Gate | Status | Value | Warning | Severe | Note |",
        "| --- | --- | ---: | ---: | ---: | --- |",
    ]
    for gate in result.gates:
        gate_lines.append(
            f"| {gate.name} | {gate.status} | {_fmt(gate.value)} {gate.unit} | "
            f"{_fmt(gate.warning)} | {_fmt(gate.severe)} | {gate.note} |"
        )
    return f"""# WAV Evaluation Report

## Inputs

- Candidate: `{candidate_path}`
- Reference: `{reference_path}`
- Metadata: `{metadata_line}`
- Profile: `{result.profile}`
- Verdict: `{result.verdict}`

## Core Metrics

| Metric | Value |
| --- | ---: |
| Candidate latency | {metrics.latency_samples} samples / {metrics.latency_ms:.3f} ms |
| Gain correction | {metrics.gain_db:.3f} dB |
| Candidate RMS | {metrics.candidate.rms_dbfs:.2f} dBFS |
| Candidate peak | {metrics.candidate.peak_dbfs:.2f} dBFS |
| Candidate DC mean | {metrics.candidate.mean_dbfs:.2f} dBFS |
| Null residual relative | {metrics.null_relative_db:.2f} dB |
| Log-spectral distance | {metrics.log_spectral_distance_db:.2f} dB |
| Weighted guitar-band LSD | {metrics.weighted_log_spectral_distance_db:.2f} dB |
| Envelope error | {metrics.envelope_error_db:.2f} dB |
| Aligned DC offset delta | {metrics.dc_offset_delta_db:.2f} dBFS |
| Near-clip samples | {result.near_clip_count} |
| Hard-clip samples | {result.hard_clip_count} |

## Gates

{chr(10).join(gate_lines)}

## Interpretation

`pass` means this profile found no gating issue. `warning` means inspect the
artifact before treating the change as an improvement. `severe` means the render
should not be promoted without explaining why the gate is expected for this
experiment.
"""


def _fmt(value: float) -> str:
    if abs(value) >= 1000.0:
        return f"{value:.0f}"
    return f"{value:.2f}"


__all__ = [
    "EvaluationGate",
    "EvaluationResult",
    "evaluate_metrics",
    "write_evaluation_json",
    "write_evaluation_report",
]
