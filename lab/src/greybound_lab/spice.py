from __future__ import annotations

import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from greybound_lab.metrics import linear_to_db, rms


@dataclass(frozen=True)
class SpiceFixture:
    name: str
    netlist_path: Path
    tmp_data_path: Path
    signals: tuple[str, ...]


@dataclass(frozen=True)
class SpiceTrace:
    time_s: np.ndarray
    signals: dict[str, np.ndarray]


@dataclass(frozen=True)
class CommonCathodeSpiceMetrics:
    plate_dc_v: float
    cathode_dc_v: float
    bplus_dc_v: float
    input_rms_v: float
    grid_rms_v: float
    plate_rms_v: float
    cathode_rms_v: float
    plate_gain: float
    plate_gain_db: float
    grid_coupling_loss_db: float


FIXTURES = {
    "common-cathode-12ax7": SpiceFixture(
        name="common-cathode-12ax7",
        netlist_path=Path("tests/fixtures/circuit/common_cathode_12ax7.cir"),
        tmp_data_path=Path("/tmp/greybound_common_cathode_12ax7.dat"),
        signals=("input", "grid", "plate", "cathode", "bplus"),
    )
}


def run_spice_fixture(name: str, output_dir: Path, repo_root: Path) -> tuple[Path, Path]:
    fixture = FIXTURES.get(name)
    if fixture is None:
        supported = ", ".join(sorted(FIXTURES))
        raise ValueError(f"unknown SPICE fixture {name!r}; supported fixtures: {supported}")

    output_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run(["ngspice", "-b", str(fixture.netlist_path)], cwd=repo_root, check=True)
    if not fixture.tmp_data_path.exists():
        raise FileNotFoundError(f"SPICE did not produce {fixture.tmp_data_path}")

    data_path = output_dir / f"{fixture.name}.dat"
    report_path = output_dir / f"{fixture.name}.md"
    shutil.copyfile(fixture.tmp_data_path, data_path)
    trace = parse_wrdata(data_path, fixture.signals)
    if fixture.name == "common-cathode-12ax7":
        metrics = common_cathode_metrics(trace)
        write_common_cathode_report(report_path, fixture, data_path, metrics)
    else:
        raise ValueError(f"no report writer for {fixture.name}")
    return data_path, report_path


def parse_wrdata(path: Path, signals: tuple[str, ...]) -> SpiceTrace:
    data = np.loadtxt(path, dtype=np.float64)
    if data.ndim != 2:
        raise ValueError(f"{path} does not contain tabular data")
    expected_columns = len(signals) * 2
    if data.shape[1] != expected_columns:
        raise ValueError(f"{path} has {data.shape[1]} columns, expected {expected_columns}")
    time_s = data[:, 0]
    parsed = {}
    for index, signal_name in enumerate(signals):
        time_column = data[:, index * 2]
        if not np.allclose(time_column, time_s, rtol=1e-7, atol=1e-12):
            raise ValueError(f"{path} has mismatched time column for {signal_name}")
        parsed[signal_name] = data[:, index * 2 + 1]
    return SpiceTrace(time_s=time_s, signals=parsed)


def common_cathode_metrics(trace: SpiceTrace, settle_time_s: float = 0.050) -> CommonCathodeSpiceMetrics:
    mask = trace.time_s >= settle_time_s
    if not np.any(mask):
        raise ValueError("SPICE trace is too short for settled metrics")
    input_v = trace.signals["input"][mask]
    grid_v = trace.signals["grid"][mask]
    plate_v = trace.signals["plate"][mask]
    cathode_v = trace.signals["cathode"][mask]
    bplus_v = trace.signals["bplus"][mask]

    input_ac = _remove_dc(input_v)
    grid_ac = _remove_dc(grid_v)
    plate_ac = _remove_dc(plate_v)
    cathode_ac = _remove_dc(cathode_v)
    input_rms = rms(input_ac)
    grid_rms = rms(grid_ac)
    plate_rms = rms(plate_ac)

    return CommonCathodeSpiceMetrics(
        plate_dc_v=float(np.mean(plate_v)),
        cathode_dc_v=float(np.mean(cathode_v)),
        bplus_dc_v=float(np.mean(bplus_v)),
        input_rms_v=input_rms,
        grid_rms_v=rms(grid_ac),
        plate_rms_v=plate_rms,
        cathode_rms_v=rms(cathode_ac),
        plate_gain=plate_rms / max(input_rms, 1.0e-12),
        plate_gain_db=linear_to_db(plate_rms / max(input_rms, 1.0e-12)),
        grid_coupling_loss_db=linear_to_db(rms(grid_ac) / max(input_rms, 1.0e-12)),
    )


def write_common_cathode_report(
    path: Path,
    fixture: SpiceFixture,
    data_path: Path,
    metrics: CommonCathodeSpiceMetrics,
) -> None:
    path.write_text(
        f"""# SPICE Fixture Report: {fixture.name}

## Inputs

- Netlist: `{fixture.netlist_path}`
- Data: `{data_path}`
- Source: ngspice batch run

## DC Operating Point

| Node | Voltage |
| --- | ---: |
| Plate | {metrics.plate_dc_v:.3f} V |
| Cathode | {metrics.cathode_dc_v:.3f} V |
| B+ | {metrics.bplus_dc_v:.3f} V |

## Settled 1 kHz Transient

Metrics are computed after the first 50 ms to avoid startup bias.

| Metric | Value |
| --- | ---: |
| Input RMS | {metrics.input_rms_v * 1000.0:.3f} mV |
| Grid RMS | {metrics.grid_rms_v * 1000.0:.3f} mV |
| Plate RMS after DC removal | {metrics.plate_rms_v * 1000.0:.3f} mV |
| Cathode RMS after DC removal | {metrics.cathode_rms_v * 1000.0:.3f} mV |
| Plate gain | {metrics.plate_gain:.2f}x |
| Plate gain | {metrics.plate_gain_db:.2f} dB |
| Grid coupling loss | {metrics.grid_coupling_loss_db:.2f} dB |

## Engineering Notes

This is a cell-level electrical reference, not a full Greybound rig reference.
Use it to validate the common-cathode stage before fitting or tuning higher-level
amp behavior.
""",
        encoding="utf-8",
    )


def _remove_dc(samples: np.ndarray) -> np.ndarray:
    return samples - np.mean(samples)
