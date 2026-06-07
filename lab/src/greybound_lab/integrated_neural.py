from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

from greybound_lab.audio import read_wav_mono
from greybound_lab.metrics import ComparisonMetrics, compare_signals
from greybound_lab.render import render_rig
from greybound_lab.segments import load_segments


SHADOW_RE = re.compile(
    r"shadow first abs err avg/max (?P<avg>[0-9.]+)/(?P<max>[0-9.]+) V n (?P<count>[0-9]+)"
)


@dataclass(frozen=True)
class IntegratedNeuralReport:
    analytic_wav: Path
    shadow_wav: Path
    replace_wav: Path
    shadow_monitor_log: Path
    replace_vs_analytic: ComparisonMetrics
    analytic_vs_reference: ComparisonMetrics | None
    replace_vs_reference: ComparisonMetrics | None
    shadow_error_avg_v: float | None
    shadow_error_max_v: float | None
    shadow_error_count: int


def evaluate_integrated_neural_cell(
    *,
    repo_root: Path,
    binary: Path,
    rig: Path,
    input_wav: Path,
    descriptor: Path,
    output_dir: Path,
    report: Path,
    component: str = "nox30.first_stage",
    render_seconds: float = 20.0,
    sample_rate_hz: int = 48_000,
    period_size: int = 16,
    input_gain_db: float = 0.0,
    output_gain_db: float = -12.0,
    ir_enabled: bool = True,
    ir_wav: Path | None = None,
    segments: Path | None = None,
    reference_wav: Path | None = None,
) -> IntegratedNeuralReport:
    output_dir.mkdir(parents=True, exist_ok=True)
    analytic_wav = output_dir / "analytic.wav"
    shadow_wav = output_dir / "shadow.wav"
    replace_wav = output_dir / "replace.wav"
    analytic_metadata = output_dir / "analytic.run.json"
    shadow_metadata = output_dir / "shadow.run.json"
    replace_metadata = output_dir / "replace.run.json"
    shadow_log = output_dir / "shadow.monitor.log"

    render_rig(
        repo_root=repo_root,
        binary=binary,
        rig=rig,
        input_wav=input_wav,
        output_wav=analytic_wav,
        metadata=analytic_metadata,
        render_seconds=render_seconds,
        sample_rate_hz=sample_rate_hz,
        period_size=period_size,
        input_gain_db=input_gain_db,
        output_gain_db=output_gain_db,
        ir_enabled=ir_enabled,
        ir_wav=ir_wav,
    )
    render_rig(
        repo_root=repo_root,
        binary=binary,
        rig=rig,
        input_wav=input_wav,
        output_wav=shadow_wav,
        metadata=shadow_metadata,
        render_seconds=render_seconds,
        sample_rate_hz=sample_rate_hz,
        period_size=period_size,
        input_gain_db=input_gain_db,
        output_gain_db=output_gain_db,
        ir_enabled=ir_enabled,
        ir_wav=ir_wav,
        monitor_enabled=True,
        monitor_log=shadow_log,
        neural_cell=(component, descriptor),
        neural_cell_mode="shadow",
    )
    render_rig(
        repo_root=repo_root,
        binary=binary,
        rig=rig,
        input_wav=input_wav,
        output_wav=replace_wav,
        metadata=replace_metadata,
        render_seconds=render_seconds,
        sample_rate_hz=sample_rate_hz,
        period_size=period_size,
        input_gain_db=input_gain_db,
        output_gain_db=output_gain_db,
        ir_enabled=ir_enabled,
        ir_wav=ir_wav,
        neural_cell=(component, descriptor),
        neural_cell_mode="replace",
    )

    analytic = read_wav_mono(analytic_wav)
    replace = read_wav_mono(replace_wav)
    if analytic.sample_rate != replace.sample_rate:
        raise ValueError("integrated neural render sample-rate mismatch")
    segment_specs = load_segments(segments) if segments else None
    metrics = compare_signals(
        replace.samples,
        analytic.samples,
        analytic.sample_rate,
        max_lag_ms=100.0,
        segments=segment_specs,
    )
    analytic_vs_reference = None
    replace_vs_reference = None
    if reference_wav is not None:
        reference = read_wav_mono(reference_wav)
        if analytic.sample_rate != reference.sample_rate or replace.sample_rate != reference.sample_rate:
            raise ValueError("integrated neural reference sample-rate mismatch")
        analytic_vs_reference = compare_signals(
            analytic.samples,
            reference.samples,
            analytic.sample_rate,
            max_lag_ms=100.0,
            segments=segment_specs,
        )
        replace_vs_reference = compare_signals(
            replace.samples,
            reference.samples,
            replace.sample_rate,
            max_lag_ms=100.0,
            segments=segment_specs,
        )
    shadow_avg, shadow_max, shadow_count = parse_shadow_error(shadow_log)
    result = IntegratedNeuralReport(
        analytic_wav=analytic_wav,
        shadow_wav=shadow_wav,
        replace_wav=replace_wav,
        shadow_monitor_log=shadow_log,
        replace_vs_analytic=metrics,
        analytic_vs_reference=analytic_vs_reference,
        replace_vs_reference=replace_vs_reference,
        shadow_error_avg_v=shadow_avg,
        shadow_error_max_v=shadow_max,
        shadow_error_count=shadow_count,
    )
    write_integrated_neural_report(report, result, component, descriptor, rig, input_wav, reference_wav)
    return result


def parse_shadow_error(path: Path) -> tuple[float | None, float | None, int]:
    if not path.exists():
        return None, None, 0
    latest: tuple[float | None, float | None, int] = (None, None, 0)
    for line in path.read_text(encoding="utf-8").splitlines():
        match = SHADOW_RE.search(line)
        if match:
            latest = (
                float(match.group("avg")),
                float(match.group("max")),
                int(match.group("count")),
            )
    return latest


def write_integrated_neural_report(
    path: Path,
    result: IntegratedNeuralReport,
    component: str,
    descriptor: Path,
    rig: Path,
    input_wav: Path,
    reference_wav: Path | None,
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    metrics = result.replace_vs_analytic
    path.write_text(
        f"""# Integrated Neural Cell Report

## Inputs

- Component: `{component}`
- Descriptor: `{descriptor}`
- Rig: `{rig}`
- Input WAV: `{input_wav}`
- Reference WAV: `{reference_wav if reference_wav else "not provided"}`
- Analytic render: `{result.analytic_wav}`
- Shadow render: `{result.shadow_wav}`
- Replace render: `{result.replace_wav}`
- Shadow monitor log: `{result.shadow_monitor_log}`

## Shadow Telemetry

- First-stage absolute error average: {_format_optional_v(result.shadow_error_avg_v)}
- First-stage absolute error max: {_format_optional_v(result.shadow_error_max_v)}
- Shadow telemetry samples: {result.shadow_error_count}

## Replace vs Analytic Audio

| Metric | Value |
| --- | ---: |
| Compared samples | {metrics.compared_samples} |
| Estimated latency | {metrics.latency_samples} samples / {metrics.latency_ms:.3f} ms |
| Gain correction | {metrics.gain_db:.3f} dB |
| Null residual RMS | {metrics.null_rms_dbfs:.2f} dBFS |
| Null residual relative | {metrics.null_relative_db:.2f} dB |
| Log-spectral distance | {metrics.log_spectral_distance_db:.2f} dB |
| Envelope error | {metrics.envelope_error_db:.2f} dB |

{_render_reference_comparisons(result.analytic_vs_reference, result.replace_vs_reference)}

## Decision

{_render_decision(result)}
""",
        encoding="utf-8",
    )


def _format_optional_v(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.6f} V"


def _render_reference_comparisons(
    analytic: ComparisonMetrics | None,
    replace: ComparisonMetrics | None,
) -> str:
    if analytic is None or replace is None:
        return ""
    return f"""## NAM Reference Comparison

| Render | Gain corr dB | Null rel dB | Log-spectral dB | Envelope dB |
| --- | ---: | ---: | ---: | ---: |
| Analytic vs NAM | {analytic.gain_db:.2f} | {analytic.null_relative_db:.2f} | {analytic.log_spectral_distance_db:.2f} | {analytic.envelope_error_db:.2f} |
| Replace vs NAM | {replace.gain_db:.2f} | {replace.null_relative_db:.2f} | {replace.log_spectral_distance_db:.2f} | {replace.envelope_error_db:.2f} |

"""


def _render_decision(result: IntegratedNeuralReport) -> str:
    lines = [
        "This report is an integration diagnostic. `shadow` measures component error without changing audio.",
        "`replace` shows how much the complete rendered chain changes when the neural counterpart feeds the rest of Nox30.",
        "",
    ]
    if result.shadow_error_avg_v is not None:
        lines.append(
            f"- Shadow first-stage average error is `{result.shadow_error_avg_v:.6f} V`; this is still too high for promotion."
        )
    lines.append(
        f"- Replace-vs-analytic null residual is `{result.replace_vs_analytic.null_relative_db:.2f} dB`, so the neural cell is audibly changing the chain."
    )
    if result.analytic_vs_reference is not None and result.replace_vs_reference is not None:
        analytic = result.analytic_vs_reference
        replace = result.replace_vs_reference
        lines.append(
            f"- Against NAM, replace changes log-spectral distance from `{analytic.log_spectral_distance_db:.2f} dB` to `{replace.log_spectral_distance_db:.2f} dB`."
        )
        lines.append(
            f"- Against NAM, replace changes null residual from `{analytic.null_relative_db:.2f} dB` to `{replace.null_relative_db:.2f} dB`."
        )
    lines.extend(
        [
            "",
            "Conclusion: keep this neural cell as a working integration probe. It is not promoted as a better Nox30 component until the local shadow error and replace residual improve materially.",
        ]
    )
    return "\n".join(lines)
