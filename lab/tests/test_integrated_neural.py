from __future__ import annotations

from pathlib import Path

import numpy as np

from greybound_lab import integrated_neural
from greybound_lab.audio import AudioBuffer
from greybound_lab.metrics import BandResidualMetrics, ComparisonMetrics, SegmentComparisonMetrics, SignalStats


def test_parse_shadow_error_uses_latest_line(tmp_path: Path) -> None:
    log = tmp_path / "monitor.log"
    log.write_text(
        "\n".join(
            [
                "ts=1 CMP n=1 shadow first abs err avg/max 0.10000/0.20000 V n 8",
                "ts=2 CMP n=1 shadow first abs err avg/max 0.03000/0.09000 V n 16",
            ]
        ),
        encoding="utf-8",
    )

    assert integrated_neural.parse_shadow_error(log) == (0.03, 0.09, 16)


def test_evaluate_integrated_neural_cell_renders_three_modes(monkeypatch, tmp_path: Path) -> None:
    calls = []

    def fake_render_rig(**kwargs):
        calls.append(kwargs)
        kwargs["output_wav"].write_bytes(b"fake")
        kwargs["metadata"].write_text("{}", encoding="utf-8")
        if kwargs.get("monitor_log"):
            kwargs["monitor_log"].write_text(
                "CMP n=1 shadow first abs err avg/max 0.01000/0.02000 V n 4\n",
                encoding="utf-8",
            )

    def fake_read_wav(path: Path) -> AudioBuffer:
        return AudioBuffer(
            path=path,
            sample_rate=48_000,
            samples=np.sin(np.linspace(0.0, 1.0, 1024, dtype=np.float64)).astype(np.float32),
        )

    monkeypatch.setattr(integrated_neural, "render_rig", fake_render_rig)
    monkeypatch.setattr(integrated_neural, "read_wav_mono", fake_read_wav)

    result = integrated_neural.evaluate_integrated_neural_cell(
        repo_root=tmp_path,
        binary=Path("target/release/greybound-cli"),
        rig=Path("rigs/nox30-driven.json5"),
        input_wav=Path("lab/references/tone3000-inputs/Brit - Guitar.wav"),
        descriptor=Path("lab/models/cell/model.greybound.json"),
        output_dir=tmp_path / "renders",
        report=tmp_path / "report.md",
        render_seconds=1.0,
        reference_wav=Path("lab/reports/nam.wav"),
    )

    assert [call.get("neural_cell_mode") for call in calls] == [None, "shadow", "replace"]
    assert calls[0].get("neural_cell") is None
    assert calls[1]["neural_cell"] == ("nox30.first_stage", Path("lab/models/cell/model.greybound.json"))
    assert result.shadow_error_avg_v == 0.01
    assert result.analytic_vs_reference is not None
    assert result.replace_vs_reference is not None
    assert "NAM Reference Comparison" in (tmp_path / "report.md").read_text(encoding="utf-8")


def test_integrated_report_renders_segment_deltas(tmp_path: Path) -> None:
    replace_vs_analytic = _comparison_with_segment("opening_attack", lsd=3.0, null=-25.0, envelope=-30.0)
    analytic_vs_reference = _comparison_with_segment("opening_attack", lsd=12.0, null=-5.0, envelope=-7.0)
    replace_vs_reference = _comparison_with_segment("opening_attack", lsd=13.5, null=-5.5, envelope=-7.2)
    result = integrated_neural.IntegratedNeuralReport(
        analytic_wav=Path("analytic.wav"),
        shadow_wav=Path("shadow.wav"),
        replace_wav=Path("replace.wav"),
        shadow_monitor_log=Path("shadow.log"),
        replace_vs_analytic=replace_vs_analytic,
        analytic_vs_reference=analytic_vs_reference,
        replace_vs_reference=replace_vs_reference,
        shadow_error_avg_v=0.1,
        shadow_error_max_v=0.2,
        shadow_error_count=16,
        program_start_s=0.35,
        analytic_vs_reference_program=analytic_vs_reference,
        replace_vs_reference_program=replace_vs_reference,
    )

    integrated_neural.write_integrated_neural_report(
        tmp_path / "report.md",
        result,
        "nox30.first_stage",
        Path("model.greybound.json"),
        Path("rig.json5"),
        Path("input.wav"),
        Path("nam.wav"),
    )
    report = (tmp_path / "report.md").read_text(encoding="utf-8")

    assert "Replace vs Analytic Segment Metrics" in report
    assert "NAM Reference Program-Material Comparison" in report
    assert "NAM Reference Segment Deltas" in report
    assert "| opening_attack | attack |" in report
    assert "1.50" in report


def _comparison_with_segment(name: str, *, lsd: float, null: float, envelope: float) -> ComparisonMetrics:
    stats = SignalStats(rms_dbfs=-20.0, peak_dbfs=-6.0, crest_db=14.0)
    segment = SegmentComparisonMetrics(
        name=name,
        kind="attack",
        start_s=0.0,
        end_s=0.35,
        samples=100,
        local_gain_db=0.0,
        null_relative_db=null,
        log_spectral_distance_db=lsd,
        envelope_error_db=envelope,
        band_residual=BandResidualMetrics(
            low_db=-40.0,
            low_mid_db=-35.0,
            mid_db=-30.0,
            presence_db=-25.0,
            air_db=-50.0,
        ),
    )
    return ComparisonMetrics(
        sample_rate_hz=48_000,
        candidate_samples=100,
        reference_samples=100,
        compared_samples=100,
        latency_samples=0,
        latency_ms=0.0,
        gain_db=0.0,
        candidate=stats,
        reference=stats,
        aligned_candidate=stats,
        aligned_reference=stats,
        null_rms_dbfs=-40.0,
        null_relative_db=null,
        log_spectral_distance_db=lsd,
        envelope_error_db=envelope,
        segments=(segment,),
    )
