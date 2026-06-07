# 001 Chain Reference Analysis

Status: planned

## Purpose

Create the first offline analysis loop for complete rig behavior. This is the
foundation for later NAM comparisons, SPICE cell replacement, and micro-model
validation.

We start with complete chains because they reveal whether model changes matter
musically. SPICE and neural training can improve local cells, but the final
question is whether a rig behaves closer to a convincing reference under real
guitar input.

## Scope

This experiment compares two WAV files:

- a Greybound render,
- a reference render or capture.

The reference may come from a NAM profile, a real amp/pedal capture, an exported
SPICE cell render, or an older Greybound render. The first implementation only
needs WAV input; NAM support can stay outside the lab until we have a stable
comparison report.

## Initial Command Shape

The first tool should eventually look like this:

```sh
lab/bin/compare-wav \
  --candidate lab/renders/nox30-driven.wav \
  --reference lab/references/nox30-reference.wav \
  --metadata lab/renders/nox30-driven.run.json \
  --report lab/reports/nox30-driven-vs-reference.md
```

The exact implementation language is not decided yet. Python is pragmatic for
analysis and plotting. Rust is preferable for reusable DSP kernels. The runtime
engine must not depend on the lab toolchain.

## Required Metadata

Every comparison needs enough metadata to make the result meaningful:

- project revision,
- rig file,
- input DI file,
- render command,
- sample rate,
- render duration,
- input gain and output gain,
- IR state and IR file if used,
- reference origin,
- known reference latency if available,
- notes about knob settings or capture conditions.

See `lab/schemas/run-metadata.schema.json` for the first schema.

## Metrics

Minimum useful metrics:

- sample rate match,
- duration match,
- peak and RMS level,
- crest factor,
- estimated latency offset,
- gain offset after alignment,
- null residual after latency and gain compensation,
- log-spectrum distance,
- transient envelope error.

Useful later metrics:

- harmonic balance on sine sweeps,
- intermodulation on multi-sine input,
- aliasing residual,
- frequency-dependent group delay,
- level-dependent dynamic response,
- control sweep continuity.

## First Reference Rig

Start with a driven Nox30-style rig because it is already central to the codebase
and exercises the full chain:

- guitar DI input from `samples/`,
- `rigs/nox30-driven.json5`,
- speaker IR enabled,
- 44.1 kHz or 48 kHz,
- fixed output gain chosen to avoid clipping.

Do not start with a high-gain distortion pedal chain. Compression, clipping, and
noise make alignment harder and can hide basic measurement mistakes.

## Decision Criteria

This experiment is successful when we can produce a report that answers:

- Are the two files aligned?
- How much gain correction was required?
- Is the error mostly broadband, spectral, transient, or dynamic?
- Is the difference large enough to justify model work?
- Which local subsystem is the likely next target?

The report does not need to say whether the model is good in absolute terms. It
needs to tell us where to investigate next.

## Follow-Up Experiments

- SPICE cell analysis for the common-cathode 12AX7 fixture.
- NAM reference comparison for a fixed Nox30-like amp setting.
- JFET/phaser cell fitting for `Tron`.
- Diode clipper comparison for `Muffin`, `Godess One`, and overdrive models.
