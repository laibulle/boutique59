from __future__ import annotations

from dataclasses import replace

from greybound_lab.metrics import ComparisonMetrics, SignalStats
from greybound_lab.rig_sweep import replace_amp_control, replace_amp_controls, sweep_score


def test_replace_amp_control_updates_control_and_name() -> None:
    rig = """{
  name: 'nox30-driven',
  amp: {
    model: 'nox30',
    controls: {
      volume: 0.76,
      drive: 0.68,
    },
  },
}
"""

    updated = replace_amp_control(rig, "drive", 0.42, "sweep-drive-0p420")

    assert "name: 'sweep-drive-0p420'," in updated
    assert "drive: 0.420000," in updated
    assert "volume: 0.76," in updated


def test_replace_amp_control_rejects_missing_control() -> None:
    try:
        replace_amp_control("{ amp: { controls: { drive: 0.5 } } }", "presence", 0.2, "generated")
    except ValueError as exc:
        assert "could not find amp.controls.presence" in str(exc)
    else:
        raise AssertionError("expected missing control to fail")


def test_replace_amp_controls_updates_multiple_controls_once() -> None:
    rig = """{
  name: 'nox30-driven',
  amp: {
    model: 'nox30',
    controls: {
      volume: 0.76,
      drive: 0.68,
      sag: 0.70,
    },
  },
}
"""

    updated = replace_amp_controls(rig, {"volume": 0.82, "drive": 0.74, "sag": 0.55}, "grid")

    assert "name: 'grid'," in updated
    assert "volume: 0.820000," in updated
    assert "drive: 0.740000," in updated
    assert "sag: 0.550000," in updated


def test_sweep_score_penalizes_weak_dynamic_match() -> None:
    base = ComparisonMetrics(
        sample_rate_hz=48_000,
        candidate_samples=48_000,
        reference_samples=48_000,
        compared_samples=48_000,
        latency_samples=0,
        latency_ms=0.0,
        gain_db=0.0,
        candidate=SignalStats(rms_dbfs=-24.0, peak_dbfs=-12.0, crest_db=12.0),
        reference=SignalStats(rms_dbfs=-24.0, peak_dbfs=-12.0, crest_db=12.0),
        aligned_candidate=SignalStats(rms_dbfs=-24.0, peak_dbfs=-12.0, crest_db=12.0),
        aligned_reference=SignalStats(rms_dbfs=-24.0, peak_dbfs=-12.0, crest_db=12.0),
        null_rms_dbfs=-30.0,
        null_relative_db=-9.0,
        log_spectral_distance_db=12.0,
        envelope_error_db=-9.0,
    )
    balanced = replace(base, log_spectral_distance_db=12.0, null_relative_db=-9.0, envelope_error_db=-9.0)
    spectral_only = replace(base, log_spectral_distance_db=11.0, null_relative_db=-3.0, envelope_error_db=-4.0)

    assert sweep_score(balanced).total < sweep_score(spectral_only).total
