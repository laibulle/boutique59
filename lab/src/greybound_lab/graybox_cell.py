from __future__ import annotations

import json
import math
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import numpy as np

from greybound_lab.render import git_revision, relative_or_absolute


@dataclass(frozen=True)
class GrayboxSequence:
    stimulus_id: str
    split: str
    kind: str
    input_v: np.ndarray
    reference_v: np.ndarray


@dataclass(frozen=True)
class GrayboxMetricRow:
    stimulus_id: str
    split: str
    kind: str
    samples: int
    reference_rms_v: float
    rmse_v: float
    mae_v: float
    max_abs_error_v: float
    relative_rmse: float


def fit_common_cathode_graybox(
    *,
    manifest_path: Path,
    output_dir: Path,
    repo_root: Path,
    epochs: int = 220,
    learning_rate: float = 8.0e-3,
    stride: int = 16,
    max_train_samples_per_stimulus: int = 2048,
    seed: int = 59,
) -> tuple[Path, Path]:
    torch = _import_torch()
    torch.manual_seed(seed)
    output_dir.mkdir(parents=True, exist_ok=True)
    config_path = output_dir / "common-cathode-graybox-state.json"
    report_path = output_dir / "common-cathode-graybox-state.md"
    sequences = load_common_cathode_sequences(manifest_path, stride=stride)
    train_sequences = [sequence for sequence in sequences if sequence.split == "train"] or sequences
    train_sequences = [
        decimate_sequence_for_training(sequence, max_train_samples_per_stimulus)
        for sequence in train_sequences
    ]
    model = CommonCathodeGrayboxTorch(torch)
    optimizer = torch.optim.AdamW(model.parameters(), lr=learning_rate)
    loss_history: list[float] = []

    for _ in range(epochs):
        optimizer.zero_grad()
        losses = []
        for sequence in train_sequences:
            x = torch.from_numpy(sequence.input_v.astype(np.float32))
            y = torch.from_numpy(sequence.reference_v.astype(np.float32))
            losses.append(torch.mean((model(x) - y) ** 2))
        loss = torch.stack(losses).mean()
        loss.backward()
        optimizer.step()
        loss_history.append(float(loss.detach().cpu().item()))

    parameters = model.export_parameters()
    rows = evaluate_graybox_sequences(parameters, sequences)
    payload = {
        "schema_version": 1,
        "artifact_id": output_dir.name,
        "model": "common-cathode-graybox-state-v0",
        "created_at": datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "dataset_manifest": relative_or_absolute(manifest_path, repo_root),
        "code_revision": git_revision(repo_root),
        "training": {
            "epochs": epochs,
            "learning_rate": learning_rate,
            "stride": stride,
            "max_train_samples_per_stimulus": max_train_samples_per_stimulus,
            "seed": seed,
            "final_train_mse": loss_history[-1] if loss_history else None,
        },
        "parameters": parameters,
        "metrics": aggregate_metrics(rows),
        "notes": (
            "Experimental gray-box probe. The structure has explicit fast and slow state "
            "updates and a bounded static nonlinearity. It is not a Greybound runtime "
            "artifact and is not approved for live audio."
        ),
    }
    config_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    write_graybox_report(report_path, payload=payload, rows=rows, loss_history=loss_history)
    return config_path, report_path


def decimate_sequence_for_training(sequence: GrayboxSequence, max_samples: int) -> GrayboxSequence:
    if max_samples <= 0 or sequence.input_v.shape[0] <= max_samples:
        return sequence
    indexes = np.linspace(0, sequence.input_v.shape[0] - 1, max_samples).round().astype(np.int64)
    return GrayboxSequence(
        stimulus_id=sequence.stimulus_id,
        split=sequence.split,
        kind=sequence.kind,
        input_v=sequence.input_v[indexes].astype(np.float32),
        reference_v=sequence.reference_v[indexes].astype(np.float32),
    )


def load_common_cathode_sequences(manifest_path: Path, *, stride: int) -> list[GrayboxSequence]:
    if stride < 1:
        raise ValueError("stride must be at least 1")
    manifest = _read_json(manifest_path)
    dataset_path = _resolve_manifest_path(manifest_path, _artifact_path(manifest, "output"))
    npz = np.load(dataset_path)
    split_by_id = _split_by_stimulus_id(manifest)
    sequences = []
    for stimulus in manifest["stimuli"]:
        stimulus_id = str(stimulus["id"])
        prefix = stimulus_id + "__"
        time_s = npz[prefix + "time_s"]
        input_v = npz[prefix + "input_v"]
        reference_v = npz[prefix + "plate_ac_v"]
        settle_time_s = float(stimulus.get("parameters", {}).get("settle_time_s", 0.0))
        mask = time_s >= settle_time_s
        sequences.append(
            GrayboxSequence(
                stimulus_id=stimulus_id,
                split=split_by_id.get(stimulus_id, "unknown"),
                kind=str(stimulus["kind"]),
                input_v=input_v[mask][::stride].astype(np.float32),
                reference_v=reference_v[mask][::stride].astype(np.float32),
            )
        )
    return sequences


def evaluate_graybox_sequences(parameters: dict[str, float], sequences: list[GrayboxSequence]) -> list[GrayboxMetricRow]:
    rows = []
    for sequence in sequences:
        prediction = run_graybox_numpy(sequence.input_v, parameters)
        error = prediction - sequence.reference_v
        reference_rms = _rms(sequence.reference_v)
        rmse = _rms(error)
        rows.append(
            GrayboxMetricRow(
                stimulus_id=sequence.stimulus_id,
                split=sequence.split,
                kind=sequence.kind,
                samples=int(sequence.reference_v.shape[0]),
                reference_rms_v=reference_rms,
                rmse_v=rmse,
                mae_v=float(np.mean(np.abs(error))) if error.size else 0.0,
                max_abs_error_v=float(np.max(np.abs(error))) if error.size else 0.0,
                relative_rmse=rmse / max(reference_rms, 1.0e-12),
            )
        )
    return rows


def run_graybox_numpy(input_v: np.ndarray, parameters: dict[str, float]) -> np.ndarray:
    fast_state = 0.0
    slow_state = 0.0
    output = np.zeros_like(input_v, dtype=np.float32)
    for index, sample in enumerate(input_v.astype(np.float32)):
        drive = (
            parameters["drive_gain"] * float(sample)
            - parameters["fast_feedback"] * fast_state
            - parameters["slow_feedback"] * slow_state
            + parameters["drive_bias"]
        )
        instant = (
            parameters["linear"] * drive
            + parameters["saturation"] * math.tanh(parameters["shape"] * drive)
            + parameters["cubic"] * drive * drive * drive
        )
        fast_state += parameters["fast_alpha"] * (instant - fast_state)
        slow_state += parameters["slow_alpha"] * (fast_state - slow_state)
        output[index] = (
            parameters["output_gain"]
            * (instant + parameters["fast_mix"] * fast_state + parameters["slow_mix"] * slow_state)
            + parameters["output_bias"]
        )
    return output


class CommonCathodeGrayboxTorch:
    def __init__(self, torch: Any) -> None:
        self.torch = torch
        self.raw_drive_gain = torch.nn.Parameter(torch.tensor(2.0))
        self.raw_shape = torch.nn.Parameter(torch.tensor(1.0))
        self.raw_fast_alpha = torch.nn.Parameter(torch.tensor(-2.0))
        self.raw_slow_alpha = torch.nn.Parameter(torch.tensor(-6.0))
        self.drive_bias = torch.nn.Parameter(torch.tensor(0.0))
        self.linear = torch.nn.Parameter(torch.tensor(-5.0))
        self.saturation = torch.nn.Parameter(torch.tensor(-1.0))
        self.cubic = torch.nn.Parameter(torch.tensor(0.0))
        self.fast_feedback = torch.nn.Parameter(torch.tensor(0.0))
        self.slow_feedback = torch.nn.Parameter(torch.tensor(0.0))
        self.fast_mix = torch.nn.Parameter(torch.tensor(0.0))
        self.slow_mix = torch.nn.Parameter(torch.tensor(0.0))
        self.output_gain = torch.nn.Parameter(torch.tensor(1.0))
        self.output_bias = torch.nn.Parameter(torch.tensor(0.0))

    def parameters(self) -> list[Any]:
        return [
            self.raw_drive_gain,
            self.raw_shape,
            self.raw_fast_alpha,
            self.raw_slow_alpha,
            self.drive_bias,
            self.linear,
            self.saturation,
            self.cubic,
            self.fast_feedback,
            self.slow_feedback,
            self.fast_mix,
            self.slow_mix,
            self.output_gain,
            self.output_bias,
        ]

    def __call__(self, input_v: Any) -> Any:
        torch = self.torch
        fast_state = torch.zeros((), dtype=input_v.dtype)
        slow_state = torch.zeros((), dtype=input_v.dtype)
        drive_gain = torch.nn.functional.softplus(self.raw_drive_gain)
        shape = torch.nn.functional.softplus(self.raw_shape) + 1.0e-6
        fast_alpha = torch.sigmoid(self.raw_fast_alpha)
        slow_alpha = torch.sigmoid(self.raw_slow_alpha)
        values = []
        for sample in input_v:
            drive = drive_gain * sample - self.fast_feedback * fast_state - self.slow_feedback * slow_state + self.drive_bias
            instant = self.linear * drive + self.saturation * torch.tanh(shape * drive) + self.cubic * drive * drive * drive
            fast_state = fast_state + fast_alpha * (instant - fast_state)
            slow_state = slow_state + slow_alpha * (fast_state - slow_state)
            values.append(self.output_gain * (instant + self.fast_mix * fast_state + self.slow_mix * slow_state) + self.output_bias)
        return torch.stack(values)

    def export_parameters(self) -> dict[str, float]:
        torch = self.torch
        with torch.no_grad():
            return {
                "drive_gain": float(torch.nn.functional.softplus(self.raw_drive_gain).cpu().item()),
                "shape": float((torch.nn.functional.softplus(self.raw_shape) + 1.0e-6).cpu().item()),
                "fast_alpha": float(torch.sigmoid(self.raw_fast_alpha).cpu().item()),
                "slow_alpha": float(torch.sigmoid(self.raw_slow_alpha).cpu().item()),
                "drive_bias": float(self.drive_bias.cpu().item()),
                "linear": float(self.linear.cpu().item()),
                "saturation": float(self.saturation.cpu().item()),
                "cubic": float(self.cubic.cpu().item()),
                "fast_feedback": float(self.fast_feedback.cpu().item()),
                "slow_feedback": float(self.slow_feedback.cpu().item()),
                "fast_mix": float(self.fast_mix.cpu().item()),
                "slow_mix": float(self.slow_mix.cpu().item()),
                "output_gain": float(self.output_gain.cpu().item()),
                "output_bias": float(self.output_bias.cpu().item()),
            }


def aggregate_metrics(rows: list[GrayboxMetricRow]) -> dict[str, float | int]:
    samples = sum(row.samples for row in rows)
    if samples == 0:
        return {"samples": 0, "weighted_rmse_v": 0.0, "weighted_mae_v": 0.0, "weighted_relative_rmse": 0.0}
    rmse = math.sqrt(sum(row.rmse_v * row.rmse_v * row.samples for row in rows) / samples)
    mae = sum(row.mae_v * row.samples for row in rows) / samples
    reference_rms = math.sqrt(sum(row.reference_rms_v * row.reference_rms_v * row.samples for row in rows) / samples)
    return {
        "samples": samples,
        "weighted_rmse_v": rmse,
        "weighted_mae_v": mae,
        "weighted_relative_rmse": rmse / max(reference_rms, 1.0e-12),
    }


def write_graybox_report(path: Path, *, payload: dict[str, Any], rows: list[GrayboxMetricRow], loss_history: list[float]) -> None:
    metrics = payload["metrics"]
    parameters = payload["parameters"]
    row_table = "\n".join(
        "| `{}` | `{}` | `{}` | {} | {:.3f} | {:.3f} | {:.3f} | {:.3f} | {:.2%} |".format(
            row.stimulus_id,
            row.split,
            row.kind,
            row.samples,
            row.reference_rms_v * 1000.0,
            row.rmse_v * 1000.0,
            row.mae_v * 1000.0,
            row.max_abs_error_v * 1000.0,
            row.relative_rmse,
        )
        for row in rows
    )
    parameter_lines = "\n".join(f"- `{key}`: `{value:.8g}`" for key, value in parameters.items())
    path.write_text(
        f"""# Common-Cathode Gray-Box Stateful Fit

## Purpose

Fit a tiny differentiable gray-box probe against the common-cathode SPICE
dataset. This follows the 2026 modulation paper's direction: explicit runtime
structure first, gradient-optimized parameters second.

This is not an accepted runtime model. It is a model-quality probe.

## Training

- Dataset manifest: `{payload['dataset_manifest']}`
- Epochs: `{payload['training']['epochs']}`
- Learning rate: `{payload['training']['learning_rate']}`
- Stride: `{payload['training']['stride']}`
- Max train samples per stimulus: `{payload['training']['max_train_samples_per_stimulus']}`
- Final train MSE: `{payload['training']['final_train_mse']:.8g}`
- Initial train MSE: `{loss_history[0] if loss_history else 0.0:.8g}`

## Aggregate

| Metric | Value |
| --- | ---: |
| Samples evaluated | {metrics['samples']} |
| Weighted RMSE | {metrics['weighted_rmse_v'] * 1000.0:.3f} mV |
| Weighted MAE | {metrics['weighted_mae_v'] * 1000.0:.3f} mV |
| Weighted relative RMSE | {metrics['weighted_relative_rmse']:.2%} |

## Parameters

{parameter_lines}

## Per-Stimulus Metrics

| Stimulus | Split | Kind | Samples | Ref RMS mV | RMSE mV | MAE mV | Max abs mV | Rel RMSE |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
{row_table}

## Decision

Compare this against the current MLP and Rust analytic reports. Promote nothing
from this report alone. If the gray-box probe wins, the next step is to map the
structure into an inspectable Rust cell. If it loses, the structure is too weak
and we should improve the physical state model rather than add opaque capacity.
""",
        encoding="utf-8",
    )


def _artifact_path(manifest: dict[str, Any], kind: str) -> str:
    for artifact in manifest["artifacts"]:
        if artifact["kind"] == kind:
            return str(artifact["path"])
    raise ValueError(f"manifest has no artifact of kind {kind!r}")


def _resolve_manifest_path(base_path: Path, path: str) -> Path:
    candidate = Path(path)
    if candidate.is_absolute():
        return candidate
    return (base_path.parent / candidate).resolve() if not candidate.exists() else candidate


def _split_by_stimulus_id(manifest: dict[str, Any]) -> dict[str, str]:
    result = {}
    for split in ("train", "validation", "test"):
        for stimulus_id in manifest["splits"].get(split, []):
            result[str(stimulus_id)] = split
    return result


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _rms(values: np.ndarray) -> float:
    return float(np.sqrt(np.mean(np.square(values.astype(np.float64))) + 1.0e-30)) if values.size else 0.0


def _import_torch() -> Any:
    try:
        import torch
    except ImportError as error:
        raise RuntimeError("PyTorch is required: run with `uv --project lab run --with torch ...`") from error
    return torch
