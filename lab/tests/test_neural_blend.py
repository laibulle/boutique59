from __future__ import annotations

from pathlib import Path

import numpy as np
from scipy.io import wavfile

from greybound_lab.neural_blend import parse_alpha_csv, run_neural_blend_sweep


def test_parse_alpha_csv() -> None:
    assert parse_alpha_csv("0,0.25,1") == [0.0, 0.25, 1.0]


def test_neural_blend_sweep_can_pick_intermediate_alpha(tmp_path: Path) -> None:
    sample_rate = 48_000
    t = np.arange(sample_rate, dtype=np.float64) / sample_rate
    analytic = np.sin(2.0 * np.pi * 440.0 * t).astype(np.float32)
    replace = (analytic * 0.5).astype(np.float32)
    reference = (analytic * 0.75).astype(np.float32)
    analytic_path = tmp_path / "analytic.wav"
    replace_path = tmp_path / "replace.wav"
    reference_path = tmp_path / "reference.wav"
    wavfile.write(analytic_path, sample_rate, analytic)
    wavfile.write(replace_path, sample_rate, replace)
    wavfile.write(reference_path, sample_rate, reference)

    points = run_neural_blend_sweep(
        analytic_wav=analytic_path,
        replace_wav=replace_path,
        reference_wav=reference_path,
        output_dir=tmp_path / "blends",
        report=tmp_path / "blend.md",
        metadata=tmp_path / "blend.json",
        alphas=[0.0, 0.5, 1.0],
    )

    best = min(points, key=lambda point: point.score.total)
    assert best.alpha == 0.5
    assert (tmp_path / "blend.md").read_text(encoding="utf-8").startswith("# Neural Blend Sweep")
    assert (tmp_path / "blend.json").exists()
