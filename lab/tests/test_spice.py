from __future__ import annotations

from pathlib import Path

import numpy as np

from greybound_lab.spice import FIXTURES, common_cathode_dataset_cases, common_cathode_dataset_manifest
from greybound_lab.spice import common_cathode_generated_netlist, common_cathode_metrics, parse_wrdata
from greybound_lab.spice import sha256_file


def test_parse_wrdata_time_value_pairs(tmp_path: Path) -> None:
    path = tmp_path / "trace.dat"
    path.write_text(
        "\n".join(
            [
                "0.0 0.0 0.0 250.0",
                "0.1 1.0 0.1 249.0",
                "0.2 0.0 0.2 250.0",
            ]
        ),
        encoding="utf-8",
    )

    trace = parse_wrdata(path, ("input", "plate"))

    assert trace.time_s.tolist() == [0.0, 0.1, 0.2]
    assert trace.signals["input"].tolist() == [0.0, 1.0, 0.0]
    assert trace.signals["plate"].tolist() == [250.0, 249.0, 250.0]


def test_common_cathode_metrics(tmp_path: Path) -> None:
    path = tmp_path / "common.dat"
    rows = []
    for index in range(100):
        time = index * 0.001
        input_v = 0.02 if index % 2 == 0 else -0.02
        grid_v = input_v * 0.98
        plate_v = 250.0 - input_v * 15.0
        cathode_v = 0.4 + input_v * 0.1
        bplus_v = 277.0
        values = [input_v, grid_v, plate_v, cathode_v, bplus_v]
        rows.append(" ".join(f"{item:.9g}" for pair in [(time, value) for value in values] for item in pair))
    path.write_text("\n".join(rows), encoding="utf-8")

    trace = parse_wrdata(path, ("input", "grid", "plate", "cathode", "bplus"))
    metrics = common_cathode_metrics(trace, settle_time_s=0.01)

    assert 249.0 < metrics.plate_dc_v < 251.0
    assert 14.0 < metrics.plate_gain < 16.0
    assert metrics.grid_coupling_loss_db < 0.0


def test_common_cathode_dataset_manifest(tmp_path: Path) -> None:
    repo_root = Path.cwd()
    fixture = FIXTURES["common-cathode-12ax7"]
    data_path = tmp_path / "common.dat"
    dataset_path = tmp_path / "common.dataset.npz"
    report_path = tmp_path / "common.md"
    rows = []
    for index in range(100):
        time = index * 0.001
        input_v = 0.02 if index % 2 == 0 else -0.02
        grid_v = input_v * 0.98
        plate_v = 250.0 - input_v * 15.0
        cathode_v = 0.4 + input_v * 0.1
        bplus_v = 277.0
        values = [input_v, grid_v, plate_v, cathode_v, bplus_v]
        rows.append(" ".join(f"{item:.9g}" for pair in [(time, value) for value in values] for item in pair))
    data_path.write_text("\n".join(rows), encoding="utf-8")
    np.savez(dataset_path, input_v=np.array([0.0, 1.0]), plate_v=np.array([250.0, 249.0]))
    report_path.write_text("# report\n", encoding="utf-8")

    trace = parse_wrdata(data_path, fixture.signals)
    metrics = common_cathode_metrics(trace, settle_time_s=0.01)
    manifest = common_cathode_dataset_manifest(
        fixture=fixture,
        repo_root=repo_root,
        data_path=data_path,
        dataset_path=dataset_path,
        report_path=report_path,
        metrics=metrics,
    )

    assert manifest["schema_version"] == 1
    assert manifest["fixture_id"] == "common-cathode-12ax7"
    assert manifest["cell_kind"] == "triode_gain_stage"
    assert manifest["sample_rate_hz"] == 1000
    assert manifest["stimuli"][0]["kind"] == "settled_sine"
    assert manifest["artifacts"][0]["sha256"] == sha256_file(dataset_path)


def test_common_cathode_dataset_cases_cover_splits(tmp_path: Path) -> None:
    cases = common_cathode_dataset_cases()
    splits = {case.split for case in cases}

    assert splits == {"train", "validation", "test"}
    assert any(case.kind == "two_tone_imd" for case in cases)
    assert any(case.kind == "dynamic_burst" and case.split == "train" for case in cases)
    assert any(case.kind == "dynamic_decay" and case.split == "test" for case in cases)
    assert any(case.kind == "dynamic_bias_recovery" and case.split == "test" for case in cases)
    assert any(case.stimulus_id == "sine_1khz_400mv" and case.split == "train" for case in cases)
    assert any(case.stimulus_id == "sine_1khz_300mv" and case.split == "validation" for case in cases)
    assert any(case.stimulus_id == "sine_1khz_120mv" and case.split == "test" for case in cases)

    dynamic_case = next(case for case in cases if case.kind == "dynamic_burst")
    netlist = common_cathode_generated_netlist(dynamic_case, tmp_path / "case.dat")
    assert "BVIN in 0" in netlist
    assert "tanh((time-0.032)" in netlist
    assert "12AX7_KOREN" in netlist
