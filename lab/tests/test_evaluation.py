from __future__ import annotations

import numpy as np

from greybound_lab.evaluation import evaluate_metrics
from greybound_lab.metrics import compare_signals
from greybound_lab.segments import SegmentSpec


def test_regression_profile_passes_matching_signals() -> None:
    sample_rate = 48_000
    reference = _sine(sample_rate, 0.25) * 0.2

    metrics = compare_signals(reference.copy(), reference, sample_rate)
    result = evaluate_metrics(metrics, reference, profile="regression")

    assert result.verdict == "pass"
    assert result.hard_clip_count == 0


def test_evaluation_flags_candidate_clipping_as_severe() -> None:
    sample_rate = 48_000
    reference = _sine(sample_rate, 0.25) * 0.2
    candidate = reference.copy()
    candidate[100:110] = 1.0

    metrics = compare_signals(candidate, reference, sample_rate)
    result = evaluate_metrics(metrics, candidate, profile="amp-tone")

    assert result.verdict == "severe"
    assert result.hard_clip_count == 10
    assert any(gate.name == "hard_clip_samples" and gate.status == "severe" for gate in result.gates)


def test_clipper_profile_flags_aliasing_segment_high_band_residual() -> None:
    sample_rate = 48_000
    time = np.arange(sample_rate, dtype=np.float64) / sample_rate
    reference = 0.1 * np.sin(2.0 * np.pi * 1_000.0 * time)
    candidate = reference + 0.02 * np.sin(2.0 * np.pi * 20_000.0 * time)

    metrics = compare_signals(
        candidate,
        reference,
        sample_rate,
        segments=[SegmentSpec(name="aliasing", start_s=0.0, end_s=1.0, kind="aliasing")],
    )
    result = evaluate_metrics(metrics, candidate, profile="clipper")

    assert result.verdict in {"warning", "severe"}
    assert any(gate.name == "aliasing.residual_high_band" for gate in result.gates)


def _sine(sample_rate: int, seconds: float) -> np.ndarray:
    time = np.arange(int(sample_rate * seconds), dtype=np.float64) / sample_rate
    return np.sin(2.0 * np.pi * 997.0 * time)
