from __future__ import annotations

import json
import re
from dataclasses import asdict, dataclass
from itertools import product
from pathlib import Path

from greybound_lab.audio import read_wav_mono
from greybound_lab.metrics import ComparisonMetrics, compare_signals
from greybound_lab.render import git_revision, render_rig
from greybound_lab.segments import SegmentSpec


@dataclass(frozen=True)
class SweepPoint:
    values: dict[str, float]
    rig_path: Path
    output_wav: Path
    metadata_path: Path
    metrics: ComparisonMetrics


@dataclass(frozen=True)
class SweepScore:
    total: float
    spectral: float
    null: float
    envelope: float
    gain: float


def run_amp_control_sweep(
    *,
    repo_root: Path,
    binary: Path,
    rig: Path,
    control: str,
    values: list[float],
    input_wav: Path,
    reference_wav: Path,
    output_dir: Path,
    report: Path,
    metadata: Path,
    render_seconds: float,
    sample_rate_hz: int,
    period_size: int,
    input_gain_db: float,
    output_gain_db: float,
    segments: list[SegmentSpec] | None = None,
    max_lag_ms: float = 100.0,
) -> list[SweepPoint]:
    return run_amp_control_grid_sweep(
        repo_root=repo_root,
        binary=binary,
        rig=rig,
        sweeps={control: values},
        input_wav=input_wav,
        reference_wav=reference_wav,
        output_dir=output_dir,
        report=report,
        metadata=metadata,
        render_seconds=render_seconds,
        sample_rate_hz=sample_rate_hz,
        period_size=period_size,
        input_gain_db=input_gain_db,
        output_gain_db=output_gain_db,
        segments=segments,
        max_lag_ms=max_lag_ms,
    )


def run_amp_control_grid_sweep(
    *,
    repo_root: Path,
    binary: Path,
    rig: Path,
    sweeps: dict[str, list[float]],
    input_wav: Path,
    reference_wav: Path,
    output_dir: Path,
    report: Path,
    metadata: Path,
    render_seconds: float,
    sample_rate_hz: int,
    period_size: int,
    input_gain_db: float,
    output_gain_db: float,
    segments: list[SegmentSpec] | None = None,
    max_lag_ms: float = 100.0,
) -> list[SweepPoint]:
    validate_sweeps(sweeps)

    output_dir.mkdir(parents=True, exist_ok=True)
    generated_rig_dir = output_dir / "generated-rigs"
    render_dir = output_dir / "renders"
    metadata_dir = output_dir / "metadata"
    generated_rig_dir.mkdir(parents=True, exist_ok=True)
    render_dir.mkdir(parents=True, exist_ok=True)
    metadata_dir.mkdir(parents=True, exist_ok=True)

    base_rig_text = rig.read_text(encoding="utf-8")
    reference = read_wav_mono(reference_wav)
    points: list[SweepPoint] = []

    controls = list(sweeps)
    combinations = [dict(zip(controls, values)) for values in product(*(sweeps[control] for control in controls))]

    for index, values in enumerate(combinations):
        label = sweep_label(values)
        generated_rig_text = replace_amp_controls(base_rig_text, values, sweep_name(label))
        generated_rig_path = generated_rig_dir / f"{index:02d}-{label}.json5"
        output_wav = render_dir / f"{index:02d}-{label}.wav"
        run_metadata = metadata_dir / f"{index:02d}-{label}.run.json"
        generated_rig_path.write_text(generated_rig_text, encoding="utf-8")

        render_rig(
            repo_root=repo_root,
            binary=binary,
            rig=Path("-"),
            rig_text=generated_rig_text,
            input_wav=input_wav,
            output_wav=output_wav,
            metadata=run_metadata,
            render_seconds=render_seconds,
            sample_rate_hz=sample_rate_hz,
            period_size=period_size,
            input_gain_db=input_gain_db,
            output_gain_db=output_gain_db,
            ir_enabled=False,
        )

        candidate = read_wav_mono(output_wav)
        if candidate.sample_rate != reference.sample_rate:
            raise ValueError(
                f"sample-rate mismatch for {output_wav}: "
                f"candidate={candidate.sample_rate} Hz reference={reference.sample_rate} Hz"
            )
        metrics = compare_signals(
            candidate.samples,
            reference.samples,
            candidate.sample_rate,
            max_lag_ms=max_lag_ms,
            segments=segments,
        )
        points.append(
            SweepPoint(
                values=values,
                rig_path=generated_rig_path,
                output_wav=output_wav,
                metadata_path=run_metadata,
                metrics=metrics,
            )
        )

    write_sweep_report(
        report,
        rig=rig,
        controls=controls,
        input_wav=input_wav,
        reference_wav=reference_wav,
        points=points,
    )
    write_sweep_metadata(
        metadata,
        repo_root=repo_root,
        rig=rig,
        controls=controls,
        sweeps=sweeps,
        input_wav=input_wav,
        reference_wav=reference_wav,
        output_dir=output_dir,
        render_seconds=render_seconds,
        sample_rate_hz=sample_rate_hz,
        period_size=period_size,
        input_gain_db=input_gain_db,
        output_gain_db=output_gain_db,
        points=points,
    )
    return points


def validate_sweeps(sweeps: dict[str, list[float]]) -> None:
    if not sweeps:
        raise ValueError("sweep needs at least one control")
    for control, values in sweeps.items():
        validate_control_name(control)
        if not values:
            raise ValueError(f"sweep for {control} needs at least one value")
        for value in values:
            if not 0.0 <= value <= 1.0:
                raise ValueError(f"sweep value {control}={value:g} is outside normalized 0.0..1.0 range")


def validate_control_name(control: str) -> None:
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", control):
        raise ValueError(f"unsupported rig control name: {control}")


def replace_amp_controls(rig_text: str, values: dict[str, float], name: str) -> str:
    for control, value in values.items():
        rig_text = replace_amp_control_value(rig_text, control, value)
    return replace_rig_name(rig_text, name)


def replace_amp_control(rig_text: str, control: str, value: float, name: str) -> str:
    return replace_amp_controls(rig_text, {control: value}, name)


def replace_amp_control_value(rig_text: str, control: str, value: float) -> str:
    validate_control_name(control)
    control_pattern = re.compile(rf"(^\s*{re.escape(control)}\s*:\s*)([-+]?\d+(?:\.\d+)?)(\s*,)", re.MULTILINE)
    rig_text, control_count = control_pattern.subn(rf"\g<1>{value:.6f}\3", rig_text, count=1)
    if control_count != 1:
        raise ValueError(f"could not find a unique `{control}` control in rig")
    return rig_text


def replace_rig_name(rig_text: str, name: str) -> str:
    name_pattern = re.compile(r"(^\s*name\s*:\s*)(['\"])(.*?)(\2)(\s*,)", re.MULTILINE)
    rig_text, name_count = name_pattern.subn(rf"\g<1>'{name}'\5", rig_text, count=1)
    if name_count == 0:
        rig_text = rig_text.replace("{", "{\n  name: '" + name + "',", 1)
    return rig_text


def sweep_name(label: str) -> str:
    return f"sweep-{label}"


def sweep_label(values: dict[str, float]) -> str:
    return "__".join(f"{control}-{value:.3f}".replace(".", "p").replace("_", "-") for control, value in values.items())


def write_sweep_report(
    path: Path,
    *,
    rig: Path,
    controls: list[str],
    input_wav: Path,
    reference_wav: Path,
    points: list[SweepPoint],
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    ranked = sorted(points, key=lambda point: sweep_score(point.metrics).total)
    best = ranked[0]
    lines = [
        "# Rig Control Sweep vs NAM Reference",
        "",
        "## Protocol",
        "",
        f"- Base rig: `{rig}`",
        f"- Swept controls: `{', '.join(controls)}`",
        f"- Input DI: `{input_wav}`",
        f"- Reference WAV: `{reference_wav}`",
        "- IR policy: `amp-head-no-ir`; Greybound is rendered without `--ir`.",
        "",
        "## Best Point",
        "",
        f"- Values: `{format_values(best.values)}`",
        f"- Composite match score: `{sweep_score(best.metrics).total:.3f}`",
        f"- Log-spectral distance: `{best.metrics.log_spectral_distance_db:.2f} dB`",
        f"- Null residual relative: `{best.metrics.null_relative_db:.2f} dB`",
        f"- Gain correction: `{best.metrics.gain_db:.2f} dB`",
        f"- WAV: `{best.output_wav}`",
        "",
        "## Top Candidates",
        "",
        "| Rank | Values | Score | Null rel dB | Log-spectral dB | Envelope dB |",
        "| ---: | --- | ---: | ---: | ---: | ---: |",
    ]
    for rank, point in enumerate(ranked[: min(5, len(ranked))], start=1):
        metrics = point.metrics
        score = sweep_score(metrics)
        lines.append(
            f"| {rank} | {format_values(point.values)} | {score.total:.3f} | {metrics.null_relative_db:.2f} | "
            f"{metrics.log_spectral_distance_db:.2f} | {metrics.envelope_error_db:.2f} |"
        )
    lines.extend(
        [
            "",
            "## Score Weights",
            "",
            "The composite score is a normalized diagnostic score where lower is better:",
            "",
            "- `45%` log-spectral distance, capped at `20 dB`.",
            "- `30%` null residual, where `-12 dB` or lower is considered good for this coarse anchor.",
            "- `20%` envelope error, where `-12 dB` or lower is considered good.",
            "- `5%` absolute gain correction, capped at `6 dB`.",
            "",
            "It is intentionally not an objective tone score. It prevents a purely spectral match from hiding weak dynamics, while preserving all raw metrics for listening-led decisions.",
            "",
        "## Sweep Table",
        "",
        "| Values | Score | Gain corr dB | Null rel dB | Log-spectral dB | Envelope dB | Candidate RMS | Candidate peak | WAV |",
        "| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
        ]
    )
    for point in points:
        metrics = point.metrics
        score = sweep_score(metrics)
        lines.append(
            f"| {format_values(point.values)} | {score.total:.3f} | {metrics.gain_db:.2f} | {metrics.null_relative_db:.2f} | "
            f"{metrics.log_spectral_distance_db:.2f} | {metrics.envelope_error_db:.2f} | "
            f"{metrics.candidate.rms_dbfs:.2f} | {metrics.candidate.peak_dbfs:.2f} | `{point.output_wav}` |"
        )
    lines.extend(
        [
            "",
            "## Interpretation",
            "",
            "This report ranks by the composite score because this sweep is meant to find a coarse control anchor without ignoring dynamics.",
            "Use the raw metrics and rendered WAVs before deciding that one point is musically superior; a responsive gain law can be musically right even when a fixed NAM snapshot prefers a nearby static setting.",
        ]
    )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_sweep_metadata(
    path: Path,
    *,
    repo_root: Path,
    rig: Path,
    controls: list[str],
    sweeps: dict[str, list[float]],
    input_wav: Path,
    reference_wav: Path,
    output_dir: Path,
    render_seconds: float,
    sample_rate_hz: int,
    period_size: int,
    input_gain_db: float,
    output_gain_db: float,
    points: list[SweepPoint],
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema_version": 1,
        "project_revision": git_revision(repo_root),
        "kind": "rig-sweep-vs-nam-reference",
        "protocol": {
            "ir_policy": "amp-head-no-ir",
            "greybound_ir_enabled": False,
        },
        "inputs": {
            "base_rig": str(rig),
            "controls": [f"amp.controls.{control}" for control in controls],
            "sweeps": sweeps,
            "input_wav": str(input_wav),
            "reference_wav": str(reference_wav),
            "output_dir": str(output_dir),
            "render_seconds": render_seconds,
            "sample_rate_hz": sample_rate_hz,
            "period_size": period_size,
            "input_gain_db": input_gain_db,
            "output_gain_db": output_gain_db,
        },
        "points": [
            {
                "values": point.values,
                "generated_rig": str(point.rig_path),
                "output_wav": str(point.output_wav),
                "metadata": str(point.metadata_path),
                "gain_db": point.metrics.gain_db,
                "match_score": sweep_score(point.metrics).total,
                "score_components": asdict(sweep_score(point.metrics)),
                "null_relative_db": point.metrics.null_relative_db,
                "log_spectral_distance_db": point.metrics.log_spectral_distance_db,
                "envelope_error_db": point.metrics.envelope_error_db,
                "candidate_rms_dbfs": point.metrics.candidate.rms_dbfs,
                "candidate_peak_dbfs": point.metrics.candidate.peak_dbfs,
            }
            for point in points
        ],
    }
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def format_values(values: dict[str, float]) -> str:
    return ", ".join(f"{control}={value:.3f}" for control, value in values.items())


def sweep_score(metrics: ComparisonMetrics) -> SweepScore:
    spectral = clamp01(metrics.log_spectral_distance_db / 20.0)
    null = clamp01((metrics.null_relative_db + 12.0) / 12.0)
    envelope = clamp01((metrics.envelope_error_db + 12.0) / 12.0)
    gain = clamp01(abs(metrics.gain_db) / 6.0)
    total = 0.45 * spectral + 0.30 * null + 0.20 * envelope + 0.05 * gain
    return SweepScore(total=total, spectral=spectral, null=null, envelope=envelope, gain=gain)


def clamp01(value: float) -> float:
    return min(1.0, max(0.0, value))
