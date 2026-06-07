import type { RuntimeConfig, RigPreset } from "./rigs";

export type MonitorStats = {
  inputRms: number;
  inputPeak: number;
  outputRms: number;
  outputPeak: number;
  inputNearClips: number;
  inputClips: number;
  outputNearClips: number;
  outputClips: number;
  inputOverruns: number;
  outputUnderruns: number;
  rails: {
    preampAvg: number;
    preampMin: number;
    piAvg: number;
    piMin: number;
    powerAvg: number;
    powerMin: number;
    screenAvg: number;
    screenMin: number;
  };
  currents: {
    firstAvg: number;
    firstMax: number;
    piAvg: number;
    piMax: number;
    powerAvg: number;
    powerMax: number;
    attackAvg: number;
    attackMax: number;
    screenAvg: number;
    screenMax: number;
  };
  cathodeAvg: number;
  cathodeMax: number;
  fluxAvg: number;
  fluxMax: number;
  probes: { label: string; avg: number; max: number }[];
};

export function dbfs(level: number) {
  return level > 0 ? 20 * Math.log10(level) : Number.NEGATIVE_INFINITY;
}

export function formatDbfs(level: number) {
  const value = dbfs(level);
  return Number.isFinite(value) ? `${value >= 0 ? "+" : ""}${value.toFixed(1)}` : "-inf";
}

export function runtimePreview(rig: RigPreset, runtime: RuntimeConfig) {
  return [
    `engine: greybound-wasm / ${rig.model}`,
    `rig: ${rig.file}`,
    `device: ${runtime.device}`,
    `input: ${runtime.inputSourceUrl}`,
    `ir: ${runtime.speakerIr ? runtime.irSourceUrl : "bypassed"}`,
    `sample-rate: ${runtime.sampleRate}`,
    `period-size: ${runtime.periodSize}`,
    `input-db: ${runtime.inputDb}`,
    `output-db: ${runtime.outputDb}`,
  ].join("\n");
}

export function simulateMonitor(rig: RigPreset, runtime: RuntimeConfig, tick: number): MonitorStats {
  const activePedals = rig.pedals.filter((pedal) => !pedal.bypassed).length;
  const gain = rig.ampBypassed ? 0.4 : rig.amp.volume + rig.amp.drive * 0.8 + activePedals * 0.11;
  const trim = Math.pow(10, runtime.outputDb / 20);
  const wave = (Math.sin(tick / 8) + 1) * 0.5;
  const inputRms = clamp(0.055 + runtime.inputDb / 900 + wave * 0.02, 0.01, 0.55);
  const inputPeak = clamp(inputRms * (2.25 + wave * 0.55), 0.02, 1.1);
  const outputRms = clamp(inputRms * (0.75 + gain) * trim * 2.3, 0.005, 0.98);
  const outputPeak = clamp(outputRms * (2.1 + rig.amp.sag), 0.01, 1.2);
  const stress = Math.max(0, outputPeak - 0.96);

  return {
    inputRms,
    inputPeak,
    outputRms,
    outputPeak,
    inputNearClips: inputPeak > 0.98 ? Math.round(6 + wave * 12) : 0,
    inputClips: inputPeak >= 1 ? Math.round(wave * 4) : 0,
    outputNearClips: outputPeak > 0.98 ? Math.round(10 + stress * 80) : 0,
    outputClips: outputPeak >= 1 ? Math.round(stress * 24) : 0,
    inputOverruns: runtime.periodSize < 64 ? 1 : 0,
    outputUnderruns: runtime.periodSize < 64 && gain > 1 ? 1 : 0,
    rails: {
      preampAvg: 292 - gain * 8,
      preampMin: 276 - gain * 16,
      piAvg: 286 - gain * 9,
      piMin: 265 - gain * 20,
      powerAvg: 318 - rig.amp.sag * 42,
      powerMin: 286 - rig.amp.sag * 70,
      screenAvg: 305 - rig.amp.sag * 36,
      screenMin: 280 - rig.amp.sag * 58,
    },
    currents: {
      firstAvg: 0.7 + rig.amp.volume * 0.8,
      firstMax: 1.4 + gain * 1.8,
      piAvg: 1.8 + rig.amp.drive * 2,
      piMax: 3.8 + gain * 3,
      powerAvg: 38 + gain * 26,
      powerMax: 82 + gain * 58,
      attackAvg: 18 + rig.amp.drive * 22,
      attackMax: 38 + gain * 44,
      screenAvg: 9 + rig.amp.sag * 24,
      screenMax: 24 + gain * 28,
    },
    cathodeAvg: 10.8 + rig.amp.sag * 2.5,
    cathodeMax: 12.4 + gain * 3.5,
    fluxAvg: 0.025 + gain * 0.014,
    fluxMax: 0.08 + gain * 0.04,
    probes: ["vol", "first", "follow", "tone", "send", "pi", "power", "ot"].map((label, index) => {
      const base = inputRms * (index + 1) * (0.4 + gain * 0.2);
      return { label, avg: base, max: base * (2.2 + wave) };
    }),
  };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
