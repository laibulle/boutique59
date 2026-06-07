Circuit fixtures
================

These fixtures are reference targets for the component-level solvers in
`core/src/circuit`. They are intentionally kept outside `core/src/amp` so the
same circuit cells can later be reused by amps, pedals, and utility stages.

`common_cathode_12ax7.cir` is a ngspice starting point for the ECC83/12AX7
common-cathode stage implemented in `circuit::triode`. It writes transient data
to `/tmp/greybound_common_cathode_12ax7.dat`. Use it to compare:

- idle plate voltage, cathode voltage, and B+ sag
- transient gain at 1 kHz with and without cathode bypass
- blocking behavior from the input coupling capacitor and grid leak

The Rust model should eventually load measured or simulated operating points
from these fixtures in regression tests. For now, this file documents the
electrical target while the in-process solver is still evolving.

Current ngspice DC operating point:

- plate: 250.54 V
- cathode: 0.40 V
- B+: 277.32 V
- grid: 0.00 V

Current 1 kHz transient reference with 20 mV sine input:

- input RMS: 14.14 mV
- plate RMS after DC removal: 210.43 mV
- plate gain: 14.88x

`cathode_follower_12ax7.cir` validates the follower cell. It writes transient
data to `/tmp/greybound_cathode_follower_12ax7.dat`.

Current ngspice cathode-follower DC operating point:

- grid: 0.00 V
- cathode: 2.63 V
- B+: 280.00 V

Current 1 kHz transient reference with 20 mV sine input:

- input RMS: 14.14 mV
- grid RMS: 14.14 mV
- cathode RMS after DC removal: 11.79 mV
- cathode gain: 0.834x

`long_tail_pair_12ax7.cir` validates the shared-cathode phase-inverter cell. It
writes transient data to `/tmp/greybound_long_tail_pair_12ax7.dat`.

Current ngspice long-tail-pair DC operating point:

- plate A: 290.37 V
- plate B: 291.94 V
- cathode/tail: 1.95 V
- grid A: 0.00 V
- grid B: 0.00 V
- B+: 300.00 V

Current 1 kHz transient reference with 20 mV sine input on grid A:

- input RMS: 14.14 mV
- grid A RMS: 14.14 mV
- grid B RMS: 0.002 mV
- plate A RMS after DC removal: 97.90 mV
- plate B RMS after DC removal: 41.77 mV
- differential plate RMS after DC removal: 139.67 mV
- differential gain: 9.88x
