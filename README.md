# Greybound

Rust real-time graybox model of Nox30, a circuit-informed approximation of a JMI-era AC30/6 with the OS/010 Top Boost unit,
implemented as a CLAP/VST3/standalone plugin with
[NIH-plug](https://github.com/robbert-vdh/nih-plug) and
[`rill-core-wdf`](https://docs.rs/rill-core-wdf).

This is not a component-exact circuit simulation. It follows the archived JMI
schematic topology using WDF RC networks, filters, and behavioral nonlinear
stages:

- bright-capped Top Boost volume and two ECC83 gain stages
- circuit-derived MNA Top Boost bass/treble network
- long-tail-pair phase inverter and post-PI Cut control
- hot cathode-biased push-pull EL84 quartet with bias shift and GZ34-like sag
- output-transformer bandwidth followed by the optional speaker IR

Model targets and reverse-engineered topology notes are in [`knowledge/models/`](knowledge/models/). Reusable circuit-component notes are in [`knowledge/circuits/`](knowledge/circuits/). Third-party schematic scans and service PDFs should not be committed to this repository.

## Engineering docs

Fumadocs is the project memory for architecture, progress, shared-state
decisions, monitor analysis, and contributor context. Authored engineering
content lives in [`knowledge/`](knowledge/); [`docs/`](docs/) is only the
Fumadocs/Next UI and build layer.

```sh
cd docs
npm run dev
npm run build
npm run typecheck
```

The local docs site starts at `http://127.0.0.1:3001` when run with
`npm run dev -- --hostname 127.0.0.1 --port 3001`.

The topology and major time constants now follow those references, but the
triodes, EL84 banks, phase inverter, transformer, and supply remain compact
behavioral models. The complete amp core runs internally at 2x sample rate
through linear-phase half-band filters to reduce nonlinear aliasing.

## Build

```sh
cargo test
cargo build --release
```

The release build produces the plugin library and a `greybound-cli` binary.
For convenient plugin bundles, install NIH-plug's `cargo xtask` bundler or add
the standard NIH-plug `xtask` crate later.

## Real-time use on macOS

The standalone binary opens the audio interface's native multichannel streams,
processes one selected guitar input, and sends the result to selected outputs.

List device names:

```sh
target/release/greybound-cli --list-devices
```

Then run with a name from the list. Channel numbers are one-based:

```sh
target/release/greybound-cli \
  --rig rigs/nox30-driven.json5 \
  --device 'Scarlett 18i8 USB' \
  --input-channel 1 \
  --output-channels 1,2 \
  --sample-rate 48000 \
  --period-size 256
```

Run the release binary directly. Adjust the device name, sample rate, and period size for the interface:

```sh
target/release/greybound-cli --rig rigs/nox30-driven.json5 --device 'Scarlett 18i8 USB' \
  --input-channel 1 --output-channels 1,2 \
  --sample-rate 44100 --period-size 128
```

The CLAP/VST3 plugin always uses the sample rate selected by its host.

If CoreAudio rejects a configuration, use an interface-supported sample rate
such as `44100`, `48000`, or `96000` and try period sizes such as `128`, `256`,
or `512`. `44000` is not a standard Scarlett sample rate. Headphones are
strongly recommended while testing.

The speaker IR is optional and disabled by default. Enable the embedded,
sample-rate-matched 200 ms Celestion Vintage 30 IR with:

```sh
target/release/greybound-cli --device 'Scarlett 18i8 USB' \
  --input-channel 1 --output-channels 1,2 \
  --sample-rate 44100 --period-size 128 --ir
```

The CLAP/VST3 plugin exposes the same feature as the default-off `Speaker IR`
parameter. It reports the fixed amp-oversampling plus 256-sample speaker-stage
latency, so switching the IR on does not change timing; when the IR is off,
convolution is skipped and only the matching dry delay runs.

## Standalone runtime controls

Rig files define amp and pedal controls. The standalone CLI requires `--rig`:

```sh
target/release/greybound-cli --rig rigs/nox30-driven.json5 \
  --device 'Scarlett 18i8 USB' --input-channel 1 --output-channels 1,2 \
  --sample-rate 44100 --period-size 16 --ir --monitor
```

`Input DB` calibrates the audio interface level before the modeled input jack.
The default is unity gain because a correctly configured, non-clipping
instrument input already provides the expected level. `Output DB` is a safety
trim after the modeled amp because the reference circuit has no master volume.

Set the Scarlett to instrument mode and adjust its hardware gain so normal hard
playing peaks around `-18` to `-12 dBFS`. Then adjust `INPUT_DB` if needed.
Add `--monitor` while testing driven sounds. It opens an interactive terminal
monitor with input/output RMS, peak dBFS, near-clip counts, hard-clip counts,
stream xruns, and live amp knobs. Use `Tab`/`Shift-Tab` to select a knob,
arrow keys to adjust it, and `q` to quit. If `output peak` approaches
`0.0 dBFS` or `output near/clip` is non-zero, lower `--output-db` before
changing the rig gain staging.
Use `--input-wav samples/teenager-electric-guitar-smooth-chords-dry_94bpm_G_major.wav`
to loop the dry guitar test file through the CLI instead of the live
input device.

Generic standalone runs:

```sh
target/release/greybound-cli --rig rigs/nox30-driven.json5 --device 'Scarlett 18i8 USB' \
  --input-channel 1 --output-channels 1,2 \
  --sample-rate 44100 --period-size 16 --ir --monitor

target/release/greybound-cli --rig rigs/nox30-driven.json5 \
  --input-wav samples/teenager-electric-guitar-smooth-chords-dry_94bpm_G_major.wav \
  --device 'Scarlett 18i8 USB' --output-channels 1,2 \
  --sample-rate 44100 --period-size 16 --ir --monitor

target/release/greybound-cli --rig rigs/nox30-driven.json5 \
  --input-wav samples/teenager-electric-guitar-smooth-chords-dry_94bpm_G_major.wav \
  --output-wav target/greybound-nox30-monitor.wav --render-seconds 10 \
  --sample-rate 44100 --period-size 16 --ir --monitor
```

File, null, and WAV monitor runs use the same binary:

```sh
target/release/greybound-cli --rig rigs/nox30-driven.json5 \
  --input-wav samples/teenager-electric-guitar-smooth-chords-dry_94bpm_G_major.wav \
  --output-wav target/greybound-nox30-monitor.wav --render-seconds 10 \
  --sample-rate 44100 --period-size 16 --ir --monitor

target/release/greybound-cli --rig rigs/muffin-nox30.json5 \
  --input-wav samples/teenager-electric-guitar-smooth-chords-dry_94bpm_G_major.wav \
  --output-wav target/greybound-fuzz-monitor.wav --render-seconds 10 \
  --sample-rate 44100 --period-size 16 --ir --monitor

target/release/greybound-cli --rig rigs/minotaur-nox30.json5 \
  --input-wav samples/teenager-electric-guitar-smooth-chords-dry_94bpm_G_major.wav \
  --output-wav target/greybound-overdrive-monitor.wav --render-seconds 10 \
  --sample-rate 44100 --period-size 16 --ir --monitor

target/release/greybound-cli --rig rigs/muffin-nox30.json5 \
  --input-wav samples/teenager-electric-guitar-smooth-chords-dry_94bpm_G_major.wav \
  --device 'Scarlett 18i8 USB' --output-channels 1,2 \
  --sample-rate 44100 --period-size 16 --ir --monitor
```

## Real-time and portability notes

- `amp::VoxAmp` is a reusable DSP core independent of the plugin and standalone
  wrappers. A future CPAL, embedded, or other device adapter can call it
  directly.
- The amp sample-processing path uses concrete types and static dispatch. The
  optional IR uses preplanned FFT trait objects once per 256-sample block.
- Neither path allocates, locks, or performs I/O in the audio callback.
- `Vec<VoxAmp>` and plugin parameter state are allocated during initialization,
  outside the audio callback.
- The nonlinear model still has a computational cost, including `tanh()` and a
  cutoff-coefficient `exp()` per sample. Benchmark the target device before
  treating it as hard real-time.
- The CPAL standalone adapter bridges CoreAudio's input and output callbacks
  with a lock-free ring buffer. Use the same interface for input and output to
  keep both streams on the same hardware clock.

## Controls

- **Top Boost Volume**: Top Boost channel volume and drive
- **Bass**: Top Boost bass control
- **Treble**: Top Boost treble control
- **Cut**: global high-frequency damping across the phase-inverter outputs
- **Output Trim**: safety output level; not present on the original amp
