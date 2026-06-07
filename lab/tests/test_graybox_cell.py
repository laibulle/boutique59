from __future__ import annotations

import json
from pathlib import Path

import numpy as np

from greybound_lab.graybox_cell import aggregate_metrics, evaluate_graybox_sequences
from greybound_lab.graybox_cell import fit_common_cathode_graybox, run_graybox_numpy
from greybound_lab.graybox_cell import GrayboxSequence


def test_run_graybox_numpy_returns_finite_signal() -> None:
    parameters = {
        "drive_gain": 2.0,
        "shape": 1.5,
        "fast_alpha": 0.2,
        "slow_alpha": 0.02,
        "drive_bias": 0.0,
        "linear": -4.0,
        "saturation": -1.0,
        "cubic": 0.0,
        "fast_feedback": 0.1,
        "slow_feedback": 0.05,
        "fast_mix": 0.2,
        "slow_mix": 0.1,
        "output_gain": 1.0,
        "output_bias": 0.0,
    }

    output = run_graybox_numpy(np.array([0.0, 0.1, -0.1], dtype=np.float32), parameters)

    assert output.shape == (3,)
    assert np.all(np.isfinite(output))


def test_evaluate_graybox_sequences_aggregates_rows() -> None:
    parameters = {
        "drive_gain": 1.0,
        "shape": 1.0,
        "fast_alpha": 1.0,
        "slow_alpha": 1.0,
        "drive_bias": 0.0,
        "linear": 1.0,
        "saturation": 0.0,
        "cubic": 0.0,
        "fast_feedback": 0.0,
        "slow_feedback": 0.0,
        "fast_mix": 0.0,
        "slow_mix": 0.0,
        "output_gain": 1.0,
        "output_bias": 0.0,
    }
    sequence = GrayboxSequence(
        stimulus_id="case_a",
        split="train",
        kind="unit",
        input_v=np.array([0.0, 0.5], dtype=np.float32),
        reference_v=np.array([0.0, 0.5], dtype=np.float32),
    )

    rows = evaluate_graybox_sequences(parameters, [sequence])
    metrics = aggregate_metrics(rows)

    assert rows[0].stimulus_id == "case_a"
    assert metrics["samples"] == 2
    assert metrics["weighted_rmse_v"] < 1.0e-12


def test_fit_common_cathode_graybox_writes_report(tmp_path: Path) -> None:
    dataset_path = tmp_path / "dataset.npz"
    manifest_path = tmp_path / "dataset.json"
    np.savez(
        dataset_path,
        case_a__time_s=np.array([0.0, 0.1, 0.2], dtype=np.float64),
        case_a__input_v=np.array([0.0, 0.1, -0.1], dtype=np.float32),
        case_a__plate_ac_v=np.array([0.0, -0.5, 0.5], dtype=np.float32),
    )
    manifest = {
        "splits": {"train": ["case_a"], "validation": [], "test": []},
        "stimuli": [
            {
                "id": "case_a",
                "kind": "unit",
                "parameters": {"settle_time_s": 0.0},
            }
        ],
        "artifacts": [{"kind": "output", "path": str(dataset_path)}],
    }
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    config_path, report_path = fit_common_cathode_graybox(
        manifest_path=manifest_path,
        output_dir=tmp_path / "fit",
        repo_root=tmp_path,
        epochs=2,
        stride=1,
    )

    assert config_path.exists()
    assert report_path.read_text(encoding="utf-8").startswith("# Common-Cathode Gray-Box")
