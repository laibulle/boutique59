# 006 SPICE To Neural Cell Plan

Status: planned

## Purpose

Define how Greybound will train small neural circuit cells from SPICE data, then
run accepted cells in the Rust audio engine without depending on Python.

This is a planning and contract document. It separates research tooling from
runtime commitments so the lab can move quickly while the live engine stays
deterministic.

## Current Decision

Use three layers:

1. PyTorch for training and research.
2. A versioned Greybound artifact format as the source of truth for accepted
   model export.
3. A specialized Rust inference implementation for real-time audio.

ONNX may be exported as a secondary inspection and compatibility artifact, but
it is not the runtime source of truth.

In short:

```text
PyTorch trains.
Greybound exports.
Rust runs.
ONNX verifies.
```

## Disclaimer And Uncertainties

This decision is based on current project constraints, not on a finished
benchmark.

Known uncertainties:

- PyTorch is the strongest research default, but exported graphs can change
  shape when model code changes. The Greybound artifact format exists to avoid
  making the live engine depend on that instability.
- ONNX is useful for inspection and external tooling, but audio cells may need
  explicit streaming state, denormal handling, control smoothing, and fixed
  buffer behavior that are easier to guarantee in our own runtime.
- Rust ML frameworks may become compelling later. For now, a general-purpose
  framework is likely more surface area than a small causal audio-cell runtime
  needs.
- SPICE is only as good as the fixture: tube models, source impedance, load
  impedance, operating point, solver tolerances, and anti-aliasing strategy can
  all bias the dataset.
- A tiny network that matches one SPICE sweep can still fail on transients,
  intermodulation, out-of-range inputs, or different operating points.
- CPU cost is not the primary research constraint, but the accepted artifact
  must eventually fit the live amplifier budget with fixed latency.

Any of these assumptions may be revised after the first complete cell benchmark.

## Why Not A General Runtime First

The runtime problem is narrower than normal machine learning inference:

- mono audio,
- causal processing,
- tiny networks,
- fixed sample rate or explicitly resampled operation,
- known control inputs,
- explicit state,
- no allocation in the audio thread,
- bounded per-sample or per-block cost,
- deterministic denormal and clipping behavior.

A general runtime can be useful in the lab, but the live engine needs fewer
features and stronger guarantees. The first Rust implementation should therefore
support only the operations we accept into Greybound artifacts.

## Artifact Boundary

An accepted neural cell is exported as:

```text
model.greybound.json
weights.greybound.bin
```

The JSON descriptor records:

- artifact schema version,
- model architecture,
- target cell kind,
- sample rate policy,
- input/output normalization,
- controls and conditioning ranges,
- recurrent or convolutional state layout,
- weight layout,
- dataset manifest hash,
- training code revision,
- validation metrics,
- latency and CPU notes,
- safety clamps and out-of-range behavior.

The binary weights file stores densely packed numeric data. The first format
should use little-endian `f32`. Later revisions may add `f16`, quantized `i16`,
or generated Rust constants if the benchmark justifies it.

## Candidate Model Families

Static or quasi-static cell:

: Use for diode clipping, simple transfer curves, and small nonlinear laws where
  memory is not physically meaningful. Candidate implementation: tiny MLP with
  explicit input/output normalization and optional control conditioning.

Causal dynamic cell:

: Use for triodes, bias effects, BBD color, and other cells where local history
  matters. Candidate implementation: small causal TCN or WaveNet-style stack
  with fixed dilation, fixed receptive field, and explicit state buffers.

Low-order state cell:

: Use for sag-like behavior or physically slow memory. Candidate implementation:
  learned or optimized low-order state update with bounded coefficients. This
  should be preferred over a large recurrent network when the physical state is
  obvious.

Table or analytic fit:

: Still valid. If a lookup table or fitted law is more stable and easier to
  inspect than a network, it should win. The neural path is a tool, not a goal.

Rejected for the first phase:

- non-causal models,
- transformer-style architectures,
- models requiring dynamic allocation,
- models with hidden lookahead,
- opaque runtime operators not implemented by Greybound.

## First Cell Target

The first milestone is the existing `common-cathode-12ax7` fixture.

Reasoning:

- it is central to amp realism,
- a first SPICE fixture already exists,
- DC operating point and small-signal behavior already have a reproducible
  anchor,
- it forces us to handle real circuit context instead of only a toy clipper.

Scope for the first pass:

- one topology,
- fixed component values,
- fixed source and load impedance,
- fixed B+,
- input/output at defined circuit nodes,
- single sample rate,
- no knob conditioning.

The first model does not need to replace the runtime Nox30 stage. It only needs
to prove the lab loop:

```text
SPICE dataset -> PyTorch model -> Greybound artifact -> Rust inference
-> Python/Rust equivalence -> metrics against held-out SPICE data.
```

## Dataset Protocol

Each SPICE dataset manifest must describe:

- fixture id,
- generated netlist hash,
- ngspice version,
- model files and component values,
- source impedance,
- load impedance,
- operating point,
- sample rate,
- anti-aliasing or oversampling policy,
- stimuli,
- target nodes,
- units,
- train/validation/test split policy,
- generated file paths and hashes.

Initial stimuli:

- level-stepped 1 kHz sine for harmonic growth,
- logarithmic sweep for broad transfer behavior,
- two-tone IMD stimulus,
- pluck-like transient stimulus,
- burst stimulus for recovery checks, even if sag is not modeled yet.

The split must hold out at least one stimulus family and at least one level
range. Otherwise we only prove interpolation inside the training examples.

## Validation Metrics

Cell-level validation should reuse lab metrics where possible:

- latency and gain alignment,
- RMS and peak error,
- log-spectral distance,
- envelope error,
- harmonic H2-H5 and THD error,
- two-tone IMD product error,
- attack timing and overshoot,
- high-band residual for aliasing triage,
- stability under inputs above the training range.

Required gates before Rust promotion:

- Python model matches held-out SPICE better than the current analytic cell or
  the baseline approximation chosen for the experiment.
- Exported Greybound artifact matches the PyTorch model within a tight numeric
  tolerance on fixed test vectors.
- Rust inference matches the exported reference within tolerance.
- The cell is stable and finite for silence, nominal guitar level, hot input,
  and out-of-range controls.
- The artifact declares fixed latency and state size.

## Rust Runtime Contract

The first runtime should implement only accepted primitives:

- dense layer,
- small activation set,
- causal convolution or explicit state update only if needed,
- optional residual connection,
- scalar control conditioning,
- input and output affine normalization,
- hard safety clamps.

Runtime rules:

- no heap allocation on the audio path,
- no dynamic graph interpretation in the audio path,
- no Python dependency,
- no filesystem access from the process callback,
- denormal-safe math,
- deterministic behavior across platforms within documented tolerance.

The Rust side may load a descriptor at initialization, validate the schema, lay
out weights into runtime structures, and then process audio with fixed buffers.

## Milestones

Milestone 1: dataset manifest contract

- Add schema for SPICE dataset manifests.
- Add schema for Greybound neural cell artifacts.
- Document where local generated datasets and model artifacts live.

Current status: implemented. `greybound-lab spice-dataset` writes the first
multi-stimulus `common-cathode-12ax7` dataset and a manifest under
`lab/datasets/spice/`.

Milestone 2: common-cathode dataset generator

- Extend `spice-run` or add a new lab command that emits stimulus/response
  arrays and a dataset manifest.
- Keep generated arrays out of git.

Current status: partially implemented. The command now generates sine-level and
two-tone IMD SPICE netlists with a train/validation/test split. The next
increment should add source/load impedance sweeps, B+ perturbation, component
tolerances, transient plucks, and real DI windows.

Milestone 3: PyTorch baseline trainer

- Train the smallest MLP that can beat a trivial baseline on static windows.
- Record validation metrics and failure cases.

Current status: implemented as an experimental smoke test. The
`train-neural-cell` command trains `common-cathode-12ax7-mlp` and exports
`model.greybound.json`, `weights.greybound.bin`, and `training-report.md`.
It is static and intentionally not accepted for runtime use.

Milestone 4: dynamic model trial

- Add a small causal TCN only if static windows fail on transient or dynamic
  behavior.
- Compare against the MLP rather than assuming the dynamic model is better.

Milestone 5: export and Rust equivalence

- Export `model.greybound.json` and `weights.greybound.bin`.
- Add a Rust loader/inference prototype behind a lab or experimental feature.
- Add golden vector tests.

Current status: partially implemented. Python/NumPy can read the exported
descriptor and weights, and the Rust core has an experimental scalar MLP loader
and inference path. The Rust side also has a preallocated `NeuralCellRuntime`
intended for audio use; generated-vector Python/Rust equivalence is available
through `export-neural-cell-vectors` and `make lab-check-neural-cell-rust`, and
that test now exercises the runtime path. Nox30 has a first-stage shadow path
behind `GREYBOUND_NOX30_FIRST_STAGE_SHADOW_DESCRIPTOR`; it runs the neural
adapter beside the analytic stage and reports monitor-log error, but it does not
feed the neural output into the audio path. Audio replacement is still pending
and must stay behind an explicit experimental gate.

Cell-level SPICE evaluation is also implemented through `evaluate-neural-cell`.
The first static MLP beats the zero baseline overall but remains weak on the hot
held-out sine case, so it is a pipeline milestone rather than a model-quality
milestone.

The Rust analytic common-cathode baseline is evaluated through
`make lab-evaluate-analytic-common-cathode`. The current analytic stage is
stronger than the first MLP on the same dataset, so neural runtime integration is
not justified yet. A diagnostic gain/latency correction only improves the
analytic baseline from about `80 mV` weighted RMSE to about `70 mV`, which means
the residual is not mostly a calibration offset. The next useful investigation is
the model mismatch itself: transfer shape, bias dynamics, discretization, and
fixture equivalence. The evaluator now includes harmonic and IMD shape sections;
the first run shows small THD/IMD deltas, so the likely missing information is
dynamic state or exact fixture behavior rather than a simple static nonlinear
curve.

Milestone 6: integration check

- Shadow a single Greybound cell in an offline render.
- Compare the complete rig against SPICE-derived expectations and NAM reference
  trends.
- Replace the cell only behind a separate explicit gate after shadow evidence
  improves.
- Promote only if the change improves the full-chain evidence.

## Decision Gates

Stop or change direction if:

- the SPICE fixture is not trustworthy enough,
- the neural model only wins by memorizing training stimuli,
- a lookup table or analytic fit gives the same result with less risk,
- Rust inference cost is too high for the live amplifier budget,
- full-chain metrics improve locally but regress musical attack, aliasing, or
  stability.

Proceed if:

- the full export loop is reproducible,
- model behavior is explainable at the cell boundary,
- metrics improve on held-out SPICE data,
- Rust inference is deterministic and bounded,
- the documentation makes the training data and limitations clear.
