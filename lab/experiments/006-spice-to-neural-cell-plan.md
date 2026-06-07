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

Current status: first intermediate trial implemented. The MLP artifact format,
Python evaluator, and Rust runtime now support a fixed causal input-history
buffer, so `weights.layout[0].in_features > 1` means the runtime evaluates
`[x[n], x[n-1], ...]` without audio-thread allocation. A local
`common-cathode-12ax7-mlp-v4-history` trial with 8 input-history samples passed
Python/Rust equivalence, but its SPICE score regressed from the current static
model (`49.555 mV` weighted RMSE) to `54.698 mV`. This does not justify full
Nox30/NAM integration. The next model-quality step should be explicit state or
a small causal TCN, not more short input-history taps on the same MLP.

Follow-up: the first explicit-state gray-box probe is now implemented through
`make lab-fit-graybox-cell`. It fits a tiny differentiable structure with fast
and slow states plus a bounded static nonlinearity, then writes a local
source-ignored report. A 120-epoch local run,
`common-cathode-12ax7-graybox-state-v0`, reached `35.207 mV` weighted RMSE on
the same SPICE evaluation set. This beats both the current static MLP and the
short-history MLP, so the next implementation step should be to map this
structure into an inspectable Rust experimental cell and compare it against the
existing analytic common-cathode report before doing full Nox30/NAM integration.
That Rust mapping is now implemented as `CommonCathodeGrayboxStateCell` and
evaluated through `make lab-evaluate-graybox-cell-rust`. The Rust evaluation
matches the Python report at `35.207 mV` weighted RMSE, `22.044 mV` weighted
MAE, and `1.88%` weighted relative RMSE at stride 16. With the analytic
baseline rerun at the same stride, the current analytic cell is `95.318 mV`
weighted RMSE (`90.418 mV` after gain/latency alignment). This promotes the
gray-box cell from a Python-only probe to the leading cell-level candidate, but
not yet to an audio-chain replacement.

Nox30 integration is available behind an explicit gray-box override:
`--graybox-cell nox30.first_stage=PATH` or
`make lab-evaluate-integrated-graybox-cell`. A first local render using
`common-cathode-12ax7-graybox-state-v0` changed the full Nox30 chain materially
(`-10.74 dB` replace-vs-analytic null residual). Against the NAM reference, it
slightly improved program-material score from `0.4956` to `0.4929` and
log-spectral distance from `10.87 dB` to `10.57 dB`, while slightly worsening
the null residual from `-5.23 dB` to `-5.13 dB`. This is enough to keep it as an
active integration probe, but not enough to promote it to the default Nox30
first stage.

Integration readiness: the validated `v0` parameters are now available as the
built-in gray-box config id `accepted`, so CLI/lab/web builds do not depend on a
local ignored model file. CLI usage is
`--graybox-cell nox30.first_stage=accepted`; the web WASM monitor exposes the
same candidate through its `First stage` selector. This makes the candidate
demoable and releasable as an explicit option while preserving the analytic
first stage as the default.

Milestone 5: export and Rust equivalence

- Export `model.greybound.json` and `weights.greybound.bin`.
- Add a Rust loader/inference prototype behind a lab or experimental feature.
- Add golden vector tests.

Current status: partially implemented. Python/NumPy can read the exported
descriptor and weights, and the Rust core has an experimental scalar MLP loader
and inference path. The Rust side also has a preallocated `NeuralCellRuntime`
intended for audio use; generated-vector Python/Rust equivalence is available
through `export-neural-cell-vectors` and `make lab-check-neural-cell-rust`, and
that test now exercises the runtime path. Nox30 has a first-stage neural path
exposed in the CLI through `--neural-cell nox30.first_stage=PATH` and
`--neural-cell-mode shadow|replace`. `shadow` runs the neural adapter beside the
analytic stage and reports monitor-log error. `replace` feeds the neural output
into the rest of Nox30, but it remains an explicit R&D diagnostic rather than an
accepted default.

Cell-level SPICE evaluation is also implemented through `evaluate-neural-cell`.
The first static MLP beats the zero baseline overall but remains weak on the hot
held-out sine case, so it is a pipeline milestone rather than a model-quality
milestone.

The Rust analytic common-cathode baseline is evaluated through
`make lab-evaluate-analytic-common-cathode`. The current analytic stage is
stronger than the first MLP but weaker than the explicit-state gray-box cell on
the same dataset. A diagnostic gain/latency correction improves the current
analytic baseline from `95.318 mV` weighted RMSE to `90.418 mV` at stride 16,
which means the residual is not mostly a calibration offset. The next useful
investigation is the model mismatch itself: transfer shape, bias dynamics,
discretization, and fixture equivalence. The evaluator now includes harmonic and
IMD shape sections; the first run shows small THD/IMD deltas, so the likely
missing information is dynamic state or exact fixture behavior rather than a
simple static nonlinear curve.

Milestone 6: integration check

- Shadow a single Greybound cell in an offline render.
- Compare the complete rig against SPICE-derived expectations and NAM reference
  trends.
- Replace the cell only with `--neural-cell-mode replace`, after shadow evidence
  improves.
- Promote only if the change improves the full-chain evidence.

Current status: implemented as a first offline integration loop through
`make lab-evaluate-integrated-neural-cell`. The loop renders analytic, shadow,
and replace versions of the same Nox30 rig, parses the monitor-log shadow error,
compares replace audio against analytic audio, and writes
`lab/reports/integrated-neural-first-stage.md`.

This is intentionally not a promotion gate yet. It proves that a Greybound neural
cell can be mounted into the Rust amp graph and evaluated in context. The next
quality gate is to reduce shadow error and replace residual enough that the
neural counterpart is better than the existing analytic cell on meaningful
stimuli.

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
