from __future__ import annotations

from pathlib import Path

from greybound_lab.spice import common_cathode_metrics, parse_wrdata


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
