from __future__ import annotations

import json
from pathlib import Path

import numpy as np

from greybound_lab.neural_cell import evaluate_neural_cell_against_spice, export_neural_cell_vectors
from greybound_lab.neural_cell import infer_artifact_numpy, infer_mlp_numpy
from greybound_lab.neural_cell import build_mlp_descriptor, PreparedDataset, PreparedSplit
from greybound_lab.neural_cell import prepare_klon_drive_clip_tone_dataset
from greybound_lab.neural_cell import read_mlp_weights, write_mlp_weights


def test_mlp_weight_roundtrip_and_numpy_inference(tmp_path: Path) -> None:
    weights_path = tmp_path / "weights.greybound.bin"
    layers = [
        {
            "weight": np.array([[2.0], [-1.0]], dtype=np.float32),
            "bias": np.array([0.5, -0.25], dtype=np.float32),
        },
        {
            "weight": np.array([[1.0, -0.5]], dtype=np.float32),
            "bias": np.array([0.1], dtype=np.float32),
        },
    ]

    write_mlp_weights(weights_path, layers)
    descriptor = {
        "weights": {
            "layout": [
                {"in_features": 1, "out_features": 2},
                {"in_features": 2, "out_features": 1},
            ]
        }
    }
    loaded = read_mlp_weights(weights_path, descriptor)
    x = np.array([[0.0], [0.5]], dtype=np.float32)

    np.testing.assert_allclose(loaded[0]["weight"], layers[0]["weight"])
    np.testing.assert_allclose(infer_mlp_numpy(x, loaded), infer_mlp_numpy(x, layers))


def test_infer_artifact_numpy_applies_normalization(tmp_path: Path) -> None:
    weights_path = tmp_path / "weights.greybound.bin"
    descriptor_path = tmp_path / "model.greybound.json"
    layers = [
        {
            "weight": np.array([[1.0]], dtype=np.float32),
            "bias": np.array([0.0], dtype=np.float32),
        }
    ]
    write_mlp_weights(weights_path, layers)
    descriptor = {
        "io": {
            "normalization": {
                "input_mean": 1.0,
                "input_std": 2.0,
                "output_mean": 10.0,
                "output_std": 4.0,
            }
        },
        "weights": {
            "path": "weights.greybound.bin",
            "layout": [{"in_features": 1, "out_features": 1}],
        },
    }
    descriptor_path.write_text(json.dumps(descriptor), encoding="utf-8")

    output = infer_artifact_numpy(descriptor_path, np.array([1.0, 3.0], dtype=np.float32))

    np.testing.assert_allclose(output, np.array([10.0, 14.0], dtype=np.float32), rtol=1e-6)


def test_infer_artifact_numpy_uses_causal_history(tmp_path: Path) -> None:
    weights_path = tmp_path / "weights.greybound.bin"
    descriptor_path = tmp_path / "model.greybound.json"
    layers = [
        {
            "weight": np.array([[1.0, 2.0]], dtype=np.float32),
            "bias": np.array([0.0], dtype=np.float32),
        }
    ]
    write_mlp_weights(weights_path, layers)
    descriptor = {
        "io": {
            "normalization": {
                "input_mean": 0.0,
                "input_std": 1.0,
                "output_mean": 0.0,
                "output_std": 1.0,
            }
        },
        "weights": {
            "path": "weights.greybound.bin",
            "layout": [{"in_features": 2, "out_features": 1}],
        },
    }
    descriptor_path.write_text(json.dumps(descriptor), encoding="utf-8")

    output = infer_artifact_numpy(descriptor_path, np.array([0.5, 0.25], dtype=np.float32))

    np.testing.assert_allclose(output, np.array([0.5, 1.25], dtype=np.float32), rtol=1e-6)


def test_export_neural_cell_vectors(tmp_path: Path) -> None:
    weights_path = tmp_path / "weights.greybound.bin"
    descriptor_path = tmp_path / "model.greybound.json"
    vectors_path = tmp_path / "equivalence-vectors.json"
    write_mlp_weights(
        weights_path,
        [
            {
                "weight": np.array([[1.0]], dtype=np.float32),
                "bias": np.array([0.0], dtype=np.float32),
            }
        ],
    )
    descriptor = {
        "artifact_id": "test-cell",
        "io": {
            "normalization": {
                "input_mean": 0.0,
                "input_std": 1.0,
                "output_mean": 0.0,
                "output_std": 1.0,
            }
        },
        "weights": {
            "path": "weights.greybound.bin",
            "layout": [{"in_features": 1, "out_features": 1}],
        },
    }
    descriptor_path.write_text(json.dumps(descriptor), encoding="utf-8")

    export_neural_cell_vectors(
        descriptor_path=descriptor_path,
        output_path=vectors_path,
        input_values=[-1.0, 0.0, 1.0],
    )
    payload = json.loads(vectors_path.read_text(encoding="utf-8"))

    assert payload["artifact_id"] == "test-cell"
    assert len(payload["cases"]) == 3
    assert payload["cases"][1]["expected_output_v"] == 0.0


def test_build_mlp_descriptor_uses_output_directory_as_artifact_id(tmp_path: Path) -> None:
    output_dir = tmp_path / "common-cathode-12ax7-mlp-v99"
    output_dir.mkdir()
    weights_path = output_dir / "weights.greybound.bin"
    weights_path.write_bytes(b"weights")
    manifest_path = tmp_path / "dataset.json"
    manifest = {"stimuli": []}
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    empty = PreparedSplit(x=np.zeros((0, 1), dtype=np.float32), y=np.zeros((0, 1), dtype=np.float32))
    prepared = PreparedDataset(
        train=empty,
        validation=empty,
        test=empty,
        input_mean=0.0,
        input_std=1.0,
        output_mean=0.0,
        output_std=1.0,
        sample_rate_hz=48000,
        history_samples=1,
        train_ids=[],
        validation_ids=[],
        test_ids=[],
    )

    descriptor = build_mlp_descriptor(
        manifest=manifest,
        manifest_path=manifest_path,
        output_dir=output_dir,
        repo_root=tmp_path,
        weights_path=weights_path,
        hidden_size=8,
        prepared=prepared,
        metrics={},
    )

    assert descriptor["artifact_id"] == "common-cathode-12ax7-mlp-v99"
    assert "experimental Nox30 integration" in descriptor["runtime"]["cpu_notes"]


def test_evaluate_neural_cell_against_spice_writes_report(tmp_path: Path) -> None:
    weights_path = tmp_path / "weights.greybound.bin"
    descriptor_path = tmp_path / "model.greybound.json"
    dataset_path = tmp_path / "dataset.npz"
    manifest_path = tmp_path / "dataset.json"
    report_path = tmp_path / "evaluation.md"
    write_mlp_weights(
        weights_path,
        [
            {
                "weight": np.array([[1.0]], dtype=np.float32),
                "bias": np.array([0.0], dtype=np.float32),
            }
        ],
    )
    descriptor = {
        "artifact_id": "test-cell",
        "io": {
            "normalization": {
                "input_mean": 0.0,
                "input_std": 1.0,
                "output_mean": 0.0,
                "output_std": 1.0,
            }
        },
        "weights": {
            "path": "weights.greybound.bin",
            "layout": [{"in_features": 1, "out_features": 1}],
        },
    }
    descriptor_path.write_text(json.dumps(descriptor), encoding="utf-8")
    np.savez(
        dataset_path,
        case_a__time_s=np.array([0.0, 0.1, 0.2], dtype=np.float64),
        case_a__input_v=np.array([0.0, 1.0, -1.0], dtype=np.float32),
        case_a__plate_ac_v=np.array([0.0, 1.0, -1.0], dtype=np.float32),
    )
    manifest = {
        "sample_rate_hz": 10,
        "splits": {"train": ["case_a"], "validation": [], "test": []},
        "stimuli": [
            {
                "id": "case_a",
                "kind": "sine_level_sweep",
                "parameters": {"settle_time_s": 0.0},
            }
        ],
        "artifacts": [{"kind": "output", "path": str(dataset_path)}],
    }
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    evaluate_neural_cell_against_spice(
        descriptor_path=descriptor_path,
        dataset_manifest_path=manifest_path,
        report_path=report_path,
        stride=1,
    )
    report = report_path.read_text(encoding="utf-8")

    assert "Neural Cell SPICE Evaluation" in report
    assert "`case_a`" in report
    assert "0.000 mV" in report


def test_prepare_klon_drive_clip_tone_dataset_adds_controls_and_history(tmp_path: Path) -> None:
    dataset_path = tmp_path / "klon.dataset.npz"
    manifest_path = tmp_path / "klon.dataset.json"
    np.savez(
        dataset_path,
        train_a__buffer_ac_v=np.array([0.0, 0.1, 0.2, 0.3], dtype=np.float32),
        train_a__clip_ac_v=np.array([0.0, 1.0, 2.0, 3.0], dtype=np.float32),
        train_a__tone_ac_v=np.array([0.0, 1.0, 2.0, 3.0], dtype=np.float32),
        validation_a__buffer_ac_v=np.array([0.4, 0.5], dtype=np.float32),
        validation_a__clip_ac_v=np.array([4.0, 5.0], dtype=np.float32),
        validation_a__tone_ac_v=np.array([4.0, 5.0], dtype=np.float32),
        test_a__buffer_ac_v=np.array([0.6, 0.7], dtype=np.float32),
        test_a__clip_ac_v=np.array([6.0, 7.0], dtype=np.float32),
        test_a__tone_ac_v=np.array([6.0, 7.0], dtype=np.float32),
    )
    manifest = {
        "sample_rate_hz": 500000,
        "splits": {"train": ["train_a"], "validation": ["validation_a"], "test": ["test_a"]},
        "stimuli": [
            {"id": "train_a", "kind": "sine", "parameters": {"gain": 0.25, "treble": 0.60, "level": 0.70}},
            {"id": "validation_a", "kind": "sine", "parameters": {"gain": 0.55, "treble": 0.30, "level": 0.70}},
            {"id": "test_a", "kind": "sine", "parameters": {"gain": 0.80, "treble": 0.85, "level": 0.70}},
        ],
        "artifacts": [{"kind": "output", "path": str(dataset_path)}],
    }
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    prepared = prepare_klon_drive_clip_tone_dataset(
        manifest_path,
        target="clip_ac_v",
        stride=1,
        history_samples=2,
    )

    assert prepared.input_ids == ["buffer_ac_v", "buffer_ac_v_t-1", "gain"]
    assert prepared.output_id == "clip_ac_v"
    assert prepared.train.x.shape == (4, 3)
    assert prepared.train.y.shape == (4, 1)
    raw_first = prepared.train.x[0] * prepared.input_std + prepared.input_mean
    np.testing.assert_allclose(raw_first, np.array([0.0, 0.0, 0.25], dtype=np.float32))
