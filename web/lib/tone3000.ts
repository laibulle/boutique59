export type Tone3000Asset = {
  id: string;
  label: string;
  fileName: string;
  url: string;
};

const REPO_RAW_BASE = "https://raw.githubusercontent.com/tone-3000/neural-amp-modeler-wasm/main/ui/public";

const INPUT_FILES = [
  "Brit - Guitar.wav",
  "Celestial - Guitar.wav",
  "Cream - Guitar.wav",
  "Decapitated - Guitar.wav",
  "Fast Thrash - Guitar.wav",
  "Fear - Guitar.wav",
  "Groove Thrash - Guitar.wav",
  "Hammer Lead - Guitar.wav",
  "Harmonics - Guitar.wav",
  "Honky - Guitar.wav",
  "Hotrod - Guitar.wav",
  "Jazz Hop - Guitar.wav",
  "Jazz Trot - Guitar.wav",
  "John - Guitar.wav",
  "Lunar - Guitar.wav",
  "Mayer - Guitar.wav",
  "Metalcore - Guitar.wav",
  "Pluck - Guitar.wav",
  "Pop Punk - Guitar.wav",
  "Power - Guitar.wav",
  "Power Thrash - Guitar.wav",
  "Progression -  Guitar.wav",
  "Raid - Guitar.wav",
  "Rotary - Guitar.wav",
  "Slide Lead - Guitar.wav",
  "Smooth - Guitar.wav",
  "Stroke - Guitar.wav",
  "Tomb - Guitar.wav",
];

const IR_FILES = ["celestion.wav", "mesa.wav", "eminence.wav", "ampeg.wav", "plate.wav", "spring.wav"];

export const tone3000Inputs = INPUT_FILES.map((fileName) => asset("inputs", fileName));
export const tone3000Irs = IR_FILES.map((fileName) => asset("irs", fileName));

export const defaultTone3000Input = tone3000Inputs[0];
export const defaultTone3000Ir = tone3000Irs[0];

function asset(kind: "inputs" | "irs", fileName: string): Tone3000Asset {
  return {
    id: fileName,
    label: fileName.replace(/\.wav$/i, ""),
    fileName,
    url: `${REPO_RAW_BASE}/${kind}/${encodeURIComponent(fileName)}`,
  };
}
