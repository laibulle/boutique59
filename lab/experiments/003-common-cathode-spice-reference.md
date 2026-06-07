# 003 Common-Cathode SPICE Reference

Status: first pass complete

## Purpose

Create the first cell-level electrical reference in the lab for the Nox30 gain
path investigation.

The controlled-stimulus rig batch showed that the most useful next target is
nonlinear gain-stage behavior. The first bounded cell is the ECC83/12AX7
common-cathode stage because:

- it is already represented in `core/src/circuit/triode.rs`,
- a SPICE fixture already exists,
- it is central to amp gain and harmonic behavior,
- it is smaller and easier to validate than a full amp chain.

## Command

```sh
uv --project lab run greybound-lab spice-run \
  --fixture common-cathode-12ax7 \
  --output-dir lab/references/spice
```

Generated local artifacts:

- `lab/references/spice/common-cathode-12ax7.dat`
- `lab/references/spice/common-cathode-12ax7.md`

These artifacts are ignored by git. The committed fixture and parser are the
source of truth.

## Reference Fixture

Netlist:

```text
tests/fixtures/circuit/common_cathode_12ax7.cir
```

Simulation:

- Koren-style 12AX7 approximation,
- 280 V raw supply,
- 10 kOhm supply resistor,
- 22 uF supply capacitor,
- 100 kOhm plate resistor,
- 1.5 kOhm cathode resistor,
- 25 uF cathode bypass capacitor,
- 22 nF input coupling capacitor,
- 1 MOhm grid leak,
- 20 mV 1 kHz sine input.

## First Metrics

DC operating point:

- plate: `250.544 V`,
- cathode: `0.402 V`,
- B+: `277.322 V`.

Settled 1 kHz transient, computed after 50 ms:

- input RMS: `14.142 mV`,
- grid RMS: `14.124 mV`,
- plate RMS after DC removal: `210.441 mV`,
- cathode RMS after DC removal: `0.013 mV`,
- plate gain: `14.88x`,
- plate gain: `23.45 dB`,
- grid coupling loss: `-0.01 dB`.

## Interpretation

This fixture gives us a small-signal electrical anchor. It does not yet answer
the large-signal THD/IMD problem from the Nox30 controlled-stimulus batch.

What it does provide:

- a reproducible SPICE import path,
- a known DC operating point,
- a known small-signal gain,
- a baseline for comparing the Rust common-cathode stage.

What it does not provide yet:

- level sweep,
- harmonic growth with input level,
- two-tone intermodulation,
- grid conduction behavior under overload,
- bias shift under repeated bursts.

## Decision

The next cell-level work should extend this fixture family from one 20 mV sine
case to the same stimulus families used at the rig level:

- sine level sweep,
- two-tone IMD,
- high-level low-frequency drive,
- burst/recovery.

Only after that should we decide whether the Rust stage needs:

- parameter tuning,
- a more explicit analytic law,
- a lookup table,
- or a fitted micro-model.
