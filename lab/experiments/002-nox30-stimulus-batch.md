# 002 Nox30 Stimulus Batch

Status: first pass complete

## Purpose

Run the first controlled-stimulus comparison between two Greybound rigs:

- reference: `rigs/nox30-clean.json5`,
- candidate: `rigs/nox30-driven.json5`.

This is not a quality judgment against a real amplifier yet. It is a lab
validation pass: the question is whether our metrics expose useful differences
between two known Greybound operating points.

## Protocol

Generate stimuli:

```sh
uv --project lab run greybound-lab generate-stimuli \
  --output-dir lab/stimuli \
  --sample-rate 44100
```

For each stimulus:

- render `nox30-clean`,
- render `nox30-driven`,
- compare driven against clean,
- use the generated marker file for segment diagnostics.

Common render settings:

- sample rate: `44100`,
- period size: `16`,
- output gain: `-18 dB`,
- IR enabled,
- render duration: `60 s`.

Generated reports:

- `lab/reports/nox30-driven-vs-clean-sine-level-sweep.md`
- `lab/reports/nox30-driven-vs-clean-two-tone-imd.md`
- `lab/reports/nox30-driven-vs-clean-aliasing-stress.md`
- `lab/reports/nox30-driven-vs-clean-sag-bursts.md`
- `lab/reports/nox30-driven-vs-clean-pluck-attacks.md`

These files are local generated artifacts and are ignored by git.

## Findings

### Harmonics

The sine-level sweep shows the strongest model difference in harmonic behavior.

100 Hz:

- at `-18 dBFS`, THD delta is `+24.94 dB`,
- at `-12 dBFS`, THD delta is `+13.44 dB`,
- at `-6 dBFS`, THD delta is `+13.98 dB`.

1 kHz:

- from `-24 dBFS` to `-6 dBFS`, THD delta stays around `+9.5` to `+10.9 dB`.

5 kHz:

- THD delta is around `+11` to `+13 dB`,
- the fifth harmonic is above Nyquist and is therefore not reported.

Interpretation:

- The driven rig is not merely louder; it changes nonlinear harmonic structure.
- The largest low-frequency jump around 100 Hz suggests the driven operating
  point is strongly level-dependent and may involve low-frequency coupling,
  bias shift, sag interaction, or excessive low-end drive into nonlinear stages.

### Intermodulation

The two-tone test shows consistent IMD increase in the driven rig.

- `440 + 550 Hz`: IMD delta `+8.04 dB`.
- `997 + 1499 Hz`: IMD delta `+10.14 dB`.
- hot `997 + 1499 Hz`: IMD delta `+10.39 dB`.

Notable products:

- hot `997 + 1499 Hz`, `2F1-F2`: `+37.82 dB`,
- `997 + 1499 Hz`, `2F2-F1`: `+32.33 dB`,
- sum products are around `+9.6` to `+9.9 dB`.

Interpretation:

- This is a useful nonlinear-stage diagnostic.
- IMD is now a better next-driver metric than global spectral distance for
  overdrive/amp work because it maps more directly to chord smear and harshness.

### High Band And Aliasing Triage

The aliasing stress test exposes several high-frequency-sensitive regions.

- `7 kHz` sine: candidate high-band is `+41.89 dB` over reference.
- `10 kHz` sine: high-band delta is small, but segment null is poor at
  `-1.27 dB`.
- `14 kHz` sine: local gain correction is extreme at `-30.72 dB`, and spectral
  distance is `37.78 dB`.
- `2 kHz -> 18 kHz` sweep: high-band delta is `+13.22 dB`.

Interpretation:

- The current high-band metric is useful as triage, but it is not yet a clean
  aliasing score.
- The next aliasing metric should separate expected harmonics from folded
  non-harmonic residuals. This matters before we make claims about oversampling
  quality.

### Sag

The sag burst test shows the first burst behaving differently from later bursts.

- `sag_burst_1`: drop delta `+19.36 dB`, recovery delta `-4.63 dB`.
- `sag_burst_2`: drop delta `-0.34 dB`, recovery delta `+0.91 dB`.
- `sag_burst_3`: drop delta `-0.29 dB`, recovery delta `+0.90 dB`.

Interpretation:

- There may be an initialization/first-event behavior worth isolating.
- Later bursts look much more stable.
- The sag metric is useful, but the stimulus should be improved with explicit
  pre-roll and repeated bursts at more levels.

### Attacks

The pluck-attack test shows timing is mostly aligned.

- peak delta is `0 ms` for the first three plucks,
- the `880 Hz` pluck is only `+0.34 ms`,
- rise deltas are near zero,
- overshoot delta ranges from about `-1.06` to `-2.48 dB`.

Interpretation:

- The driven rig changes attack amplitude/overshoot more than timing.
- Attack timing is probably not the first subsystem to investigate.

## Decision

The first serious model-analysis target should be nonlinear gain-stage behavior,
using harmonic and intermodulation stimuli as the primary metrics.

Priority order:

1. Triode/gain-stage nonlinear behavior and level-dependent coupling.
2. Stronger aliasing metric that separates expected harmonics from folded
   non-harmonic residuals.
3. Sag stimulus refinement with pre-roll and repeated bursts.
4. Attack analysis can wait; timing is already close for these stimuli.

## Next Step

Create a focused cell-level experiment for the Nox30 gain path:

- render current Greybound response on sine-level and two-tone stimuli,
- compare against either SPICE fixtures or a more detailed local circuit target,
- use THD and IMD deltas as the main acceptance metrics,
- only then decide whether the correction should be analytic, table-based, or a
  fitted micro-model.
