from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

from greybound_lab.audio import read_wav_mono
from greybound_lab.metrics import ComparisonMetrics, compare_signals
from greybound_lab.render import render_rig
from greybound_lab.segments import SegmentSpec, load_segments


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
    program_start_s: float | None = None
    analytic_vs_reference_program: ComparisonMetrics | None = None
    replace_vs_reference_program: ComparisonMetrics | None = None


@dataclass(frozen=True)
class NamMatchScore:
    total: float
    spectral: float
    null: float
    envelope: float
    gain: float


def evaluate_integrated_neural_cell(
    *,
    repo_root: Path,
    binary: Path,
    rig: Path,
    input_wav: Path,
    output_dir: Path,
    report: Path,
    descriptor: Path | None = None,
    graybox_config: Path | None = None,
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
    if descriptor is None and graybox_config is None:
        raise ValueError("descriptor or graybox_config is required")
    if descriptor is not None and graybox_config is not None:
        raise ValueError("descriptor and graybox_config are mutually exclusive")
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
        disable_neural_cell=True,
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
        neural_cell=(component, descriptor) if descriptor is not None else None,
        graybox_cell=(component, graybox_config) if graybox_config is not None else None,
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
        neural_cell=(component, descriptor) if descriptor is not None else None,
        graybox_cell=(component, graybox_config) if graybox_config is not None else None,
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
    analytic_vs_reference_program = None
    replace_vs_reference_program = None
    program_start_s = _program_material_start_s(segment_specs)
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
        if program_start_s is not None and program_start_s > 0.0:
            start = int(round(program_start_s * analytic.sample_rate))
            analytic_vs_reference_program = compare_signals(
                analytic.samples[start:],
                reference.samples[start:],
                analytic.sample_rate,
                max_lag_ms=100.0,
            )
            replace_vs_reference_program = compare_signals(
                replace.samples[start:],
                reference.samples[start:],
                replace.sample_rate,
                max_lag_ms=100.0,
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
        program_start_s=program_start_s,
        analytic_vs_reference_program=analytic_vs_reference_program,
        replace_vs_reference_program=replace_vs_reference_program,
    )
    write_integrated_neural_report(
        report,
        result,
        component,
        descriptor,
        graybox_config,
        rig,
        input_wav,
        reference_wav,
    )
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
    descriptor: Path | None,
    graybox_config: Path | None,
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
- Descriptor: `{descriptor if descriptor else "not provided"}`
- Gray-box config: `{graybox_config if graybox_config else "not provided"}`
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

{_render_single_segment_table("Replace vs Analytic Segment Metrics", metrics)}

{_render_reference_comparisons(result.analytic_vs_reference, result.replace_vs_reference)}
{_render_nam_score_table("NAM Match Score", result.analytic_vs_reference, result.replace_vs_reference)}
{_render_program_reference_comparisons(result)}
{_render_nam_score_table("NAM Program-Material Match Score", result.analytic_vs_reference_program, result.replace_vs_reference_program)}
{_render_reference_segment_deltas(result.analytic_vs_reference, result.replace_vs_reference)}

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


def _render_program_reference_comparisons(result: IntegratedNeuralReport) -> str:
    analytic = result.analytic_vs_reference_program
    replace = result.replace_vs_reference_program
    if analytic is None or replace is None or result.program_start_s is None:
        return ""
    return f"""## NAM Reference Program-Material Comparison

This excludes explicit preroll before `{result.program_start_s:.3f} s`.

| Render | Gain corr dB | Null rel dB | Log-spectral dB | Envelope dB |
| --- | ---: | ---: | ---: | ---: |
| Analytic vs NAM | {analytic.gain_db:.2f} | {analytic.null_relative_db:.2f} | {analytic.log_spectral_distance_db:.2f} | {analytic.envelope_error_db:.2f} |
| Replace vs NAM | {replace.gain_db:.2f} | {replace.null_relative_db:.2f} | {replace.log_spectral_distance_db:.2f} | {replace.envelope_error_db:.2f} |

"""


def _render_nam_score_table(
    title: str,
    analytic: ComparisonMetrics | None,
    replace: ComparisonMetrics | None,
) -> str:
    if analytic is None or replace is None:
        return ""
    analytic_score = nam_match_score(analytic)
    replace_score = nam_match_score(replace)
    delta = replace_score.total - analytic_score.total
    winner = "replace" if delta < 0.0 else "analytic"
    return f"""## {title}

Lower is better. Weights: `45%` log-spectral, `30%` null residual, `20%` envelope, `5%` gain correction.

| Render | Score | Spectral | Null | Envelope | Gain |
| --- | ---: | ---: | ---: | ---: | ---: |
| Analytic vs NAM | {analytic_score.total:.4f} | {analytic_score.spectral:.4f} | {analytic_score.null:.4f} | {analytic_score.envelope:.4f} | {analytic_score.gain:.4f} |
| Replace vs NAM | {replace_score.total:.4f} | {replace_score.spectral:.4f} | {replace_score.null:.4f} | {replace_score.envelope:.4f} | {replace_score.gain:.4f} |

- Score delta replace-analytic: `{delta:+.4f}`. Current winner: `{winner}`.

"""


def _render_single_segment_table(title: str, metrics: ComparisonMetrics) -> str:
    if not metrics.segments:
        return ""
    lines = [
        f"## {title}",
        "",
        "| Segment | Kind | Time s | Gain dB | Null rel dB | Log-spectral dB | Envelope dB |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    for segment in metrics.segments:
        lines.append(
            f"| {segment.name} | {segment.kind} | {segment.start_s:.3f}-{segment.end_s:.3f} | "
            f"{segment.local_gain_db:.2f} | {segment.null_relative_db:.2f} | "
            f"{segment.log_spectral_distance_db:.2f} | {segment.envelope_error_db:.2f} |"
        )
    lines.append("")
    return "\n".join(lines)


def _program_material_start_s(segments: list[SegmentSpec] | None) -> float | None:
    if not segments:
        return None
    starts = [segment.start_s for segment in segments if segment.kind.lower() != "preroll"]
    return min(starts) if starts else None


def _render_reference_segment_deltas(
    analytic: ComparisonMetrics | None,
    replace: ComparisonMetrics | None,
) -> str:
    if analytic is None or replace is None or not analytic.segments or not replace.segments:
        return ""
    replace_by_name = {segment.name: segment for segment in replace.segments}
    lines = [
        "## NAM Reference Segment Deltas",
        "",
        "Negative deltas mean the neural replacement moved that segment closer to NAM. Positive deltas mean it moved away.",
        "",
        "| Segment | Kind | Analytic LSD | Replace LSD | LSD delta | Analytic null | Replace null | Null delta | Analytic env | Replace env | Env delta |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
    ]
    for analytic_segment in analytic.segments:
        replace_segment = replace_by_name.get(analytic_segment.name)
        if replace_segment is None:
            continue
        lines.append(
            f"| {analytic_segment.name} | {analytic_segment.kind} | "
            f"{analytic_segment.log_spectral_distance_db:.2f} | {replace_segment.log_spectral_distance_db:.2f} | "
            f"{replace_segment.log_spectral_distance_db - analytic_segment.log_spectral_distance_db:.2f} | "
            f"{analytic_segment.null_relative_db:.2f} | {replace_segment.null_relative_db:.2f} | "
            f"{replace_segment.null_relative_db - analytic_segment.null_relative_db:.2f} | "
            f"{analytic_segment.envelope_error_db:.2f} | {replace_segment.envelope_error_db:.2f} | "
            f"{replace_segment.envelope_error_db - analytic_segment.envelope_error_db:.2f} |"
        )
    lines.append("")
    return "\n".join(lines)


def _render_decision(result: IntegratedNeuralReport) -> str:
    lines = [
        "This report is an integration diagnostic. `shadow` measures component error without changing audio.",
        "`replace` shows how much the complete rendered chain changes when the neural counterpart feeds the rest of Nox30.",
        "",
    ]
    if result.shadow_error_avg_v is not None:
        lines.append(
            f"- Shadow first-stage average error is `{result.shadow_error_avg_v:.6f} V`; use this for component debugging, not as the primary promotion gate."
        )
    replace_null = result.replace_vs_analytic.null_relative_db
    if replace_null > -18.0:
        replace_note = "the neural cell is still strongly changing the rendered chain."
    elif replace_null > -30.0:
        replace_note = "the neural cell is much closer to the analytic chain, but the residual is still material."
    else:
        replace_note = "the neural cell is close enough to the analytic chain to justify deeper listening and segment review."
    lines.append(f"- Replace-vs-analytic null residual is `{replace_null:.2f} dB`; {replace_note}")
    if result.analytic_vs_reference is not None and result.replace_vs_reference is not None:
        analytic = result.analytic_vs_reference
        replace = result.replace_vs_reference
        analytic_score = nam_match_score(analytic)
        replace_score = nam_match_score(replace)
        score_delta = replace_score.total - analytic_score.total
        if score_delta < 0.0:
            score_note = "replace is better on the weighted NAM score."
        else:
            score_note = "analytic is still better on the weighted NAM score."
        lines.append(
            f"- Weighted NAM score changes from `{analytic_score.total:.4f}` to `{replace_score.total:.4f}` (`{score_delta:+.4f}`); {score_note}"
        )
        lines.append(
            f"- Against NAM, replace changes log-spectral distance from `{analytic.log_spectral_distance_db:.2f} dB` to `{replace.log_spectral_distance_db:.2f} dB`."
        )
        lines.append(
            f"- Against NAM, replace changes null residual from `{analytic.null_relative_db:.2f} dB` to `{replace.null_relative_db:.2f} dB`."
        )
    if result.analytic_vs_reference_program is not None and result.replace_vs_reference_program is not None:
        analytic_program = result.analytic_vs_reference_program
        replace_program = result.replace_vs_reference_program
        analytic_program_score = nam_match_score(analytic_program)
        replace_program_score = nam_match_score(replace_program)
        lines.append(
            f"- Excluding preroll, NAM log-spectral distance changes from `{analytic_program.log_spectral_distance_db:.2f} dB` to `{replace_program.log_spectral_distance_db:.2f} dB`."
        )
        lines.append(
            f"- Excluding preroll, weighted NAM score changes from `{analytic_program_score.total:.4f}` to `{replace_program_score.total:.4f}`."
        )
    lines.extend(
        [
            "",
            "Conclusion: keep this neural cell as a working integration probe. Promotion is NAM-first: the replacement should improve the weighted NAM score while replace-vs-analytic remains a stability guardrail.",
        ]
    )
    return "\n".join(lines)


def nam_match_score(metrics: ComparisonMetrics) -> NamMatchScore:
    spectral = _clamp01(metrics.log_spectral_distance_db / 20.0)
    null = _clamp01((metrics.null_relative_db + 12.0) / 12.0)
    envelope = _clamp01((metrics.envelope_error_db + 12.0) / 12.0)
    gain = _clamp01(abs(metrics.gain_db) / 6.0)
    total = 0.45 * spectral + 0.30 * null + 0.20 * envelope + 0.05 * gain
    return NamMatchScore(total=total, spectral=spectral, null=null, envelope=envelope, gain=gain)


def _clamp01(value: float) -> float:
    return min(1.0, max(0.0, value))
