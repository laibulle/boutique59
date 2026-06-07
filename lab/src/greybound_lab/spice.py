from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path

import numpy as np

from greybound_lab.metrics import linear_to_db, rms
from greybound_lab.render import git_revision, relative_or_absolute


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


@dataclass(frozen=True)
class CommonCathodeDatasetCase:
    stimulus_id: str
    kind: str
    expression: str
    parameters: dict[str, float | str]
    split: str
    settle_time_s: float = 0.030
    transient_stop_s: float = 0.060
    transient_step_s: float = 1.0e-6


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


def write_spice_dataset(
    name: str,
    output_dir: Path,
    repo_root: Path,
) -> tuple[Path, Path]:
    fixture = FIXTURES.get(name)
    if fixture is None:
        supported = ", ".join(sorted(FIXTURES))
        raise ValueError(f"unknown SPICE fixture {name!r}; supported fixtures: {supported}")
    if fixture.name != "common-cathode-12ax7":
        raise ValueError(f"no dataset writer for {fixture.name}")

    output_dir.mkdir(parents=True, exist_ok=True)
    netlist_dir = output_dir / "netlists"
    trace_dir = output_dir / "traces"
    netlist_dir.mkdir(parents=True, exist_ok=True)
    trace_dir.mkdir(parents=True, exist_ok=True)

    cases = common_cathode_dataset_cases()
    traces: dict[str, SpiceTrace] = {}
    raw_paths: dict[str, Path] = {}
    netlist_paths: dict[str, Path] = {}
    for case in cases:
        netlist_path = netlist_dir / f"{case.stimulus_id}.cir"
        raw_path = trace_dir / f"{case.stimulus_id}.dat"
        netlist_path.write_text(
            common_cathode_generated_netlist(case, raw_path),
            encoding="utf-8",
        )
        subprocess.run(["ngspice", "-b", str(netlist_path)], cwd=repo_root, check=True)
        if not raw_path.exists():
            raise FileNotFoundError(f"SPICE did not produce {raw_path}")
        raw_paths[case.stimulus_id] = raw_path
        netlist_paths[case.stimulus_id] = netlist_path
        traces[case.stimulus_id] = parse_wrdata(raw_path, fixture.signals)

    reference_case = "sine_1khz_20mv"
    trace = traces[reference_case]
    metrics = common_cathode_metrics(trace, settle_time_s=0.030)
    dataset_path = output_dir / f"{fixture.name}.dataset.npz"
    manifest_path = output_dir / f"{fixture.name}.dataset.json"
    report_path = output_dir / f"{fixture.name}.dataset.md"

    arrays = {}
    for stimulus_id, case_trace in traces.items():
        prefix = stimulus_id + "__"
        arrays[prefix + "time_s"] = case_trace.time_s.astype(np.float64)
        arrays[prefix + "input_v"] = case_trace.signals["input"].astype(np.float64)
        arrays[prefix + "grid_v"] = case_trace.signals["grid"].astype(np.float64)
        arrays[prefix + "plate_v"] = case_trace.signals["plate"].astype(np.float64)
        arrays[prefix + "cathode_v"] = case_trace.signals["cathode"].astype(np.float64)
        arrays[prefix + "bplus_v"] = case_trace.signals["bplus"].astype(np.float64)
        arrays[prefix + "plate_ac_v"] = _remove_dc(case_trace.signals["plate"]).astype(np.float64)
    np.savez(dataset_path, **arrays)

    write_common_cathode_dataset_report(report_path, fixture, cases, metrics)
    manifest = common_cathode_sweep_dataset_manifest(
        fixture=fixture,
        repo_root=repo_root,
        cases=cases,
        raw_paths=raw_paths,
        netlist_paths=netlist_paths,
        dataset_path=dataset_path,
        report_path=report_path,
        metrics=metrics,
    )
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return dataset_path, manifest_path


def common_cathode_dataset_cases() -> list[CommonCathodeDatasetCase]:
    return [
        CommonCathodeDatasetCase(
            stimulus_id="sine_1khz_5mv",
            kind="sine_level_sweep",
            expression="0.005*sin(2*pi*1000*time)",
            parameters={"frequency_hz": 1000.0, "amplitude_v": 0.005},
            split="train",
        ),
        CommonCathodeDatasetCase(
            stimulus_id="sine_1khz_20mv",
            kind="sine_level_sweep",
            expression="0.020*sin(2*pi*1000*time)",
            parameters={"frequency_hz": 1000.0, "amplitude_v": 0.020},
            split="train",
        ),
        CommonCathodeDatasetCase(
            stimulus_id="sine_1khz_80mv",
            kind="sine_level_sweep",
            expression="0.080*sin(2*pi*1000*time)",
            parameters={"frequency_hz": 1000.0, "amplitude_v": 0.080},
            split="train",
        ),
        CommonCathodeDatasetCase(
            stimulus_id="sine_1khz_400mv",
            kind="sine_level_sweep",
            expression="0.400*sin(2*pi*1000*time)",
            parameters={"frequency_hz": 1000.0, "amplitude_v": 0.400},
            split="train",
        ),
        CommonCathodeDatasetCase(
            stimulus_id="sine_1khz_40mv",
            kind="sine_level_sweep",
            expression="0.040*sin(2*pi*1000*time)",
            parameters={"frequency_hz": 1000.0, "amplitude_v": 0.040},
            split="validation",
        ),
        CommonCathodeDatasetCase(
            stimulus_id="sine_1khz_300mv",
            kind="sine_level_sweep",
            expression="0.300*sin(2*pi*1000*time)",
            parameters={"frequency_hz": 1000.0, "amplitude_v": 0.300},
            split="validation",
        ),
        CommonCathodeDatasetCase(
            stimulus_id="sine_1khz_120mv",
            kind="sine_level_sweep",
            expression="0.120*sin(2*pi*1000*time)",
            parameters={"frequency_hz": 1000.0, "amplitude_v": 0.120},
            split="test",
        ),
        CommonCathodeDatasetCase(
            stimulus_id="two_tone_997_1499_20mv",
            kind="two_tone_imd",
            expression="0.010*sin(2*pi*997*time)+0.010*sin(2*pi*1499*time)",
            parameters={"first_hz": 997.0, "second_hz": 1499.0, "combined_peak_v": 0.020},
            split="train",
        ),
        CommonCathodeDatasetCase(
            stimulus_id="two_tone_997_1499_80mv",
            kind="two_tone_imd",
            expression="0.040*sin(2*pi*997*time)+0.040*sin(2*pi*1499*time)",
            parameters={"first_hz": 997.0, "second_hz": 1499.0, "combined_peak_v": 0.080},
            split="test",
        ),
        CommonCathodeDatasetCase(
            stimulus_id="sine_burst_1khz_80mv",
            kind="dynamic_burst",
            expression=(
                "0.080*sin(2*pi*1000*time)"
                "*(0.5+0.5*tanh((time-0.032)/0.0003))"
                "*(0.5-0.5*tanh((time-0.052)/0.0003))"
            ),
            parameters={
                "frequency_hz": 1000.0,
                "amplitude_v": 0.080,
                "event_start_s": 0.032,
                "event_stop_s": 0.052,
                "edge_time_s": 0.0003,
            },
            split="train",
            transient_stop_s=0.080,
        ),
        CommonCathodeDatasetCase(
            stimulus_id="sine_burst_1khz_40mv",
            kind="dynamic_burst",
            expression=(
                "0.040*sin(2*pi*1000*time)"
                "*(0.5+0.5*tanh((time-0.032)/0.0003))"
                "*(0.5-0.5*tanh((time-0.052)/0.0003))"
            ),
            parameters={
                "frequency_hz": 1000.0,
                "amplitude_v": 0.040,
                "event_start_s": 0.032,
                "event_stop_s": 0.052,
                "edge_time_s": 0.0003,
            },
            split="validation",
            transient_stop_s=0.080,
        ),
        CommonCathodeDatasetCase(
            stimulus_id="pluck_decay_750hz_90mv",
            kind="dynamic_decay",
            expression=(
                "0.090*sin(2*pi*750*time)"
                "*exp(-(time-0.032)/0.018)"
                "*(0.5+0.5*tanh((time-0.032)/0.0003))"
            ),
            parameters={
                "frequency_hz": 750.0,
                "amplitude_v": 0.090,
                "event_start_s": 0.032,
                "decay_time_s": 0.018,
                "edge_time_s": 0.0003,
            },
            split="test",
            transient_stop_s=0.100,
        ),
        CommonCathodeDatasetCase(
            stimulus_id="bias_recovery_probe_20mv_after_400mv",
            kind="dynamic_bias_recovery",
            expression=(
                "0.020*sin(2*pi*1000*time)"
                "*(0.5+0.5*tanh((time-0.032)/0.0003))"
                "*(0.5-0.5*tanh((time-0.052)/0.0003))"
                "+0.400*sin(2*pi*1000*time)"
                "*(0.5+0.5*tanh((time-0.060)/0.0003))"
                "*(0.5-0.5*tanh((time-0.130)/0.0003))"
                "+0.020*sin(2*pi*1000*time)"
                "*(0.5+0.5*tanh((time-0.150)/0.0003))"
                "*(0.5-0.5*tanh((time-0.190)/0.0003))"
            ),
            parameters={
                "frequency_hz": 1000.0,
                "probe_amplitude_v": 0.020,
                "stress_amplitude_v": 0.400,
                "pre_probe_start_s": 0.032,
                "pre_probe_stop_s": 0.052,
                "stress_start_s": 0.060,
                "stress_stop_s": 0.130,
                "post_probe_start_s": 0.150,
                "post_probe_stop_s": 0.190,
                "edge_time_s": 0.0003,
            },
            split="test",
            transient_stop_s=0.210,
        ),
    ]


def common_cathode_generated_netlist(case: CommonCathodeDatasetCase, raw_path: Path) -> str:
    return f"""* Generated Greybound common-cathode dataset case: {case.stimulus_id}
.param BRAW=280
.param pi=3.141592653589793

VRAW raw 0 DC {{BRAW}}
RSUP raw bplus 10k
CSUP bplus 0 22u IC={{BRAW}}

BVIN in 0 V={{ {case.expression} }}
CIN in grid 22n
RGRID grid 0 1Meg

RPLATE bplus plate 100k
RK cath 0 1.5k
CK cath 0 25u
XTRIODE plate grid cath 12AX7_KOREN

.save v(in) v(grid) v(plate) v(cath) v(bplus)

.control
set filetype=ascii
tran {case.transient_step_s:g} {case.transient_stop_s:g} 0 {case.transient_step_s:g}
wrdata {raw_path.resolve()} v(in) v(grid) v(plate) v(cath) v(bplus)
quit
.endc

.subckt 12AX7_KOREN P G K
.param MU=100 EX=1.4 KG1=1060 KP=600 KVB=300
E1 n1 0 VALUE={{ln(1 + exp(KP * (1 / MU + V(G,K) / max(V(P,K), 1)))) / KP}}
G1 P K VALUE={{(V(P,K) / KG1) * pow(max(V(n1), 0), EX) * sqrt(max(V(P,K), 0) / KVB)}}
Cpk P K 1.7p
Cgp G P 1.6p
Cgk G K 1.6p
.ends 12AX7_KOREN

.end
"""


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


def common_cathode_dataset_manifest(
    *,
    fixture: SpiceFixture,
    repo_root: Path,
    data_path: Path,
    dataset_path: Path,
    report_path: Path,
    metrics: CommonCathodeSpiceMetrics,
) -> dict:
    return {
        "schema_version": 1,
        "dataset_id": fixture.name + "-settled-sine-v1",
        "fixture_id": fixture.name,
        "cell_kind": "triode_gain_stage",
        "created_at": datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "generator": {
            "name": "greybound-lab spice-dataset",
            "version": "0.1.0",
            "git_revision": git_revision(repo_root),
        },
        "spice": {
            "engine": "ngspice",
            "version": _ngspice_version(repo_root),
            "options": {
                "filetype": "ascii",
                "transient_step_s": 1.0e-6,
                "transient_stop_s": 0.100,
            },
        },
        "circuit": {
            "netlist_sha256": sha256_file(repo_root / fixture.netlist_path),
            "source_impedance_ohm": 0.0,
            "load_impedance_ohm": 1_000_000.0,
            "operating_point": {
                "plate_dc_v": metrics.plate_dc_v,
                "cathode_dc_v": metrics.cathode_dc_v,
                "bplus_dc_v": metrics.bplus_dc_v,
            },
            "components": {
                "tube_model": "12AX7_KOREN",
                "vin": "SIN(0 20m 1k)",
                "input_coupling_cap_f": 22.0e-9,
                "grid_leak_ohm": 1_000_000.0,
                "plate_resistor_ohm": 100_000.0,
                "cathode_resistor_ohm": 1_500.0,
                "cathode_bypass_cap_f": 25.0e-6,
                "raw_supply_v": 280.0,
                "supply_resistor_ohm": 10_000.0,
                "supply_cap_f": 22.0e-6,
            },
        },
        "sample_rate_hz": _sample_rate_from_trace(data_path, fixture.signals),
        "oversampling": {
            "factor": 1,
            "filter": "none",
        },
        "stimuli": [
            {
                "id": "settled_1khz_20mv_sine",
                "kind": "settled_sine",
                "path": relative_or_absolute(data_path, repo_root),
                "sha256": sha256_file(data_path),
                "parameters": {
                    "frequency_hz": 1000.0,
                    "amplitude_v": 0.020,
                    "settle_time_s": 0.050,
                },
            }
        ],
        "targets": [
            {"node": "in", "unit": "V", "role": "input"},
            {"node": "grid", "unit": "V", "role": "state"},
            {"node": "plate", "unit": "V", "role": "output"},
            {"node": "cathode", "unit": "V", "role": "state"},
            {"node": "bplus", "unit": "V", "role": "reference"},
        ],
        "splits": {
            "train": ["settled_1khz_20mv_sine"],
            "validation": [],
            "test": [],
            "policy": "Bootstrap dataset only. Future datasets must hold out stimulus families and level ranges.",
        },
        "artifacts": [
            {
                "path": relative_or_absolute(dataset_path, repo_root),
                "kind": "output",
                "sha256": sha256_file(dataset_path),
            },
            {
                "path": relative_or_absolute(report_path, repo_root),
                "kind": "report",
                "sha256": sha256_file(report_path),
            },
        ],
        "notes": (
            "Bootstrap dataset from the first common-cathode fixture. It is useful "
            "for testing the data/export loop, but it is not sufficient for training "
            "a robust neural cell."
        ),
    }


def common_cathode_sweep_dataset_manifest(
    *,
    fixture: SpiceFixture,
    repo_root: Path,
    cases: list[CommonCathodeDatasetCase],
    raw_paths: dict[str, Path],
    netlist_paths: dict[str, Path],
    dataset_path: Path,
    report_path: Path,
    metrics: CommonCathodeSpiceMetrics,
) -> dict:
    train = [case.stimulus_id for case in cases if case.split == "train"]
    validation = [case.stimulus_id for case in cases if case.split == "validation"]
    test = [case.stimulus_id for case in cases if case.split == "test"]
    artifacts = [
        {
            "path": relative_or_absolute(dataset_path, repo_root),
            "kind": "output",
            "sha256": sha256_file(dataset_path),
        },
        {
            "path": relative_or_absolute(report_path, repo_root),
            "kind": "report",
            "sha256": sha256_file(report_path),
        },
    ]
    for case in cases:
        artifacts.append(
            {
                "path": relative_or_absolute(netlist_paths[case.stimulus_id], repo_root),
                "kind": "netlist",
                "sha256": sha256_file(netlist_paths[case.stimulus_id]),
            }
        )

    return {
        "schema_version": 1,
        "dataset_id": fixture.name + "-sweep-current",
        "fixture_id": fixture.name,
        "cell_kind": "triode_gain_stage",
        "created_at": datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "generator": {
            "name": "greybound-lab spice-dataset",
            "version": "0.1.0",
            "git_revision": git_revision(repo_root),
        },
        "spice": {
            "engine": "ngspice",
            "version": _ngspice_version(repo_root),
            "options": {
                "filetype": "ascii",
                "transient_step_s": 1.0e-6,
                "transient_stop_s": 0.060,
            },
        },
        "circuit": {
            "netlist_sha256": sha256_file(netlist_paths[cases[0].stimulus_id]),
            "source_impedance_ohm": 0.0,
            "load_impedance_ohm": 1_000_000.0,
            "operating_point": {
                "plate_dc_v": metrics.plate_dc_v,
                "cathode_dc_v": metrics.cathode_dc_v,
                "bplus_dc_v": metrics.bplus_dc_v,
            },
            "components": {
                "tube_model": "12AX7_KOREN",
                "input_coupling_cap_f": 22.0e-9,
                "grid_leak_ohm": 1_000_000.0,
                "plate_resistor_ohm": 100_000.0,
                "cathode_resistor_ohm": 1_500.0,
                "cathode_bypass_cap_f": 25.0e-6,
                "raw_supply_v": 280.0,
                "supply_resistor_ohm": 10_000.0,
                "supply_cap_f": 22.0e-6,
            },
        },
        "sample_rate_hz": _sample_rate_from_trace(raw_paths[cases[0].stimulus_id], fixture.signals),
        "oversampling": {
            "factor": 1,
            "filter": "none",
        },
        "stimuli": [
            {
                "id": case.stimulus_id,
                "kind": case.kind,
                "path": relative_or_absolute(raw_paths[case.stimulus_id], repo_root),
                "sha256": sha256_file(raw_paths[case.stimulus_id]),
                "parameters": {
                    **case.parameters,
                    "transient_stop_s": case.transient_stop_s,
                    "settle_time_s": case.settle_time_s,
                },
            }
            for case in cases
        ],
        "targets": [
            {"node": "in", "unit": "V", "role": "input"},
            {"node": "grid", "unit": "V", "role": "state"},
            {"node": "plate", "unit": "V", "role": "output"},
            {"node": "cathode", "unit": "V", "role": "state"},
            {"node": "bplus", "unit": "V", "role": "reference"},
        ],
        "splits": {
            "train": train,
            "validation": validation,
            "test": test,
            "policy": (
                "Train covers low/nominal/hot sine plus a nominal two-tone case. "
                "Training also includes a deliberately high 400 mV sine so the "
                "bias-recovery stress test is not merely an amplitude extrapolation "
                "case. Validation holds out intermediate 40 mV and 300 mV sine "
                "levels. Test holds out an extra-hot sine and a hotter two-tone IMD "
                "case. Dynamic burst and decay cases probe whether static curve "
                "fits survive onset and release behavior. The bias recovery probe "
                "repeats the same small signal before and after a hot stress window "
                "to expose state memory."
            ),
        },
        "artifacts": artifacts,
        "notes": (
            "First multi-stimulus common-cathode dataset. It is suitable for a "
            "baseline MLP/TCN training smoke test and now includes first dynamic "
            "burst/decay and bias-recovery probes. It still lacks source/load "
            "impedance sweeps, B+ perturbation, component tolerances, and real DI."
        ),
    }


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


def write_common_cathode_dataset_report(
    path: Path,
    fixture: SpiceFixture,
    cases: list[CommonCathodeDatasetCase],
    metrics: CommonCathodeSpiceMetrics,
) -> None:
    rows = "\n".join(
        f"| `{case.stimulus_id}` | `{case.kind}` | `{case.split}` | `{case.expression}` |"
        for case in cases
    )
    path.write_text(
        f"""# SPICE Dataset Report: {fixture.name}

## Purpose

This dataset is the first multi-stimulus common-cathode corpus for Greybound's
SPICE-to-neural-cell workflow. It is intended for baseline training and export
smoke tests, not for final tube-stage model acceptance.

## Fixture

- Cell: 12AX7/ECC83 common-cathode gain stage
- Plate resistor: `100k`
- Cathode resistor: `1.5k`
- Cathode bypass capacitor: `25u`
- Input coupling capacitor: `22n`
- Grid leak: `1Meg`
- Raw supply: `280 V`
- Supply resistor: `10k`
- SPICE model: Koren-style `12AX7_KOREN`

## Reference Operating Point

Computed from the held nominal `sine_1khz_20mv` case after settling.

| Node | Voltage |
| --- | ---: |
| Plate | {metrics.plate_dc_v:.3f} V |
| Cathode | {metrics.cathode_dc_v:.3f} V |
| B+ | {metrics.bplus_dc_v:.3f} V |

| Metric | Value |
| --- | ---: |
| Input RMS | {metrics.input_rms_v * 1000.0:.3f} mV |
| Plate RMS after DC removal | {metrics.plate_rms_v * 1000.0:.3f} mV |
| Plate gain | {metrics.plate_gain:.2f}x |
| Plate gain | {metrics.plate_gain_db:.2f} dB |

## Stimuli

| Stimulus | Kind | Split | Expression |
| --- | --- | --- | --- |
{rows}

## Limitations

- Source impedance is still idealized at `0 ohm`.
- Load is still the grid leak / fixture context, not a downstream tone stack.
- B+ is fixed; there is no supply perturbation or sag corpus yet.
- Component tolerances are not swept.
- The corpus does not include real DI phrases yet.

Use this dataset to prove the training/export/runtime loop before drawing
conclusions about final model quality.
""",
        encoding="utf-8",
    )


def _remove_dc(samples: np.ndarray) -> np.ndarray:
    return samples - np.mean(samples)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _sample_rate_from_trace(path: Path, signals: tuple[str, ...]) -> int:
    trace = parse_wrdata(path, signals)
    if trace.time_s.shape[0] < 2:
        raise ValueError("SPICE trace is too short to infer sample rate")
    step_s = float(np.median(np.diff(trace.time_s)))
    if step_s <= 0.0:
        raise ValueError("SPICE trace has non-positive time step")
    return int(round(1.0 / step_s))


def _ngspice_version(repo_root: Path) -> str:
    try:
        result = subprocess.run(
            ["ngspice", "--version"],
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        return "unknown"
    return (result.stdout or result.stderr).strip().splitlines()[0]
