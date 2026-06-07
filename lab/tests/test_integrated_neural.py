from __future__ import annotations

from pathlib import Path

import numpy as np

from greybound_lab import integrated_neural
from greybound_lab.audio import AudioBuffer


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
