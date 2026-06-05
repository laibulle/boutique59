# Nox network coverage

Nox is structurally complete enough to start grey-box component work. The model
is not SPICE-accurate yet, but every major amp section has an explicit component
boundary and observable state.

## Replaceable boundaries

- `input_volume`: input coupling, volume attenuation, bright bypass
- `first_stage`: nonlinear ECC83 common-cathode gain stage
- `cathode_follower`: nonlinear ECC83 follower
- `tone_stack`: top-boost passive tone network
- `drive_stage`: nonlinear ECC83 drive stage
- `recovery_stage`: nonlinear ECC83 recovery stage
- `phase_inverter`: nonlinear long-tail-pair phase inverter
- `cut_presence`: cut and presence shaping network
- `power_stage`: push-pull EL84 plate-feedback stage
- `supply_network`: shared B+ rail and sag network
- `output_transformer`: output transformer filtering and core-flux state

## Observable state

`VoxAmp::nox_operating_point()` exposes the current Nox operating point for
debugging and later grey-box training:

- preamp, phase-inverter, and power rail voltages
- ECC83 stage currents and cathode voltages
- long-tail-pair currents and cathode voltage
- EL84 currents and cathode bias
- output transformer core flux

## Current validation level

The network is validated by deterministic behavior tests and block-level
fixtures:

- sample WAV render stays in a fixed RMS/peak/checksum band
- Nox does not go silent over an 8-second looping sample render
- shared supply rails sag and recover
- transformer blocks DC, rolls off highs, and compresses low-end drive
- triode stages still match the current ngspice fixture tolerances

This is the baseline for replacing individual components with grey-box or
neural-WDF implementations.
