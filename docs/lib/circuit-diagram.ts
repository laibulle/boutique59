export type CircuitDiagramConfidence = "high" | "medium" | "low" | "inferred";

export type CircuitDiagramSpice = {
  primitive: string;
  name: string;
  nodes: string[];
  value?: string;
  model?: string;
  params?: Record<string, string | number>;
} | null;

export type CircuitDiagramNode = {
  id: string;
  label: string;
  kind: string;
  ports?: string[];
  hardware?: {
    role?: string;
    parts?: string[];
    value?: string;
    confidence?: CircuitDiagramConfidence;
    notes?: string[];
  };
  emulation?: {
    implementation?: string;
    library?: string[];
    state?: string[];
    algorithm?: string;
    notes?: string[];
  };
  spice?: CircuitDiagramSpice;
  layout?: {
    x: number;
    y: number;
    width?: number;
    height?: number;
  };
};

export type CircuitDiagramEdge = {
  from: string;
  to: string;
  signal?: string;
  label?: string;
  spiceNet?: string;
};

export type CircuitDiagramGroup = {
  id: string;
  label: string;
  nodes: string[];
};

export type CircuitDiagram = {
  schema: "greybound.circuit-diagram.v1";
  model: string;
  title: string;
  status: "documentation" | "draft" | "validated";
  sourceOfTruth: "rust-model" | "reference-selection" | "measured-hardware";
  renderer?: {
    layout?: "left-to-right" | "freeform";
    preferredWidth?: number;
    preferredHeight?: number;
  };
  nodes: CircuitDiagramNode[];
  edges: CircuitDiagramEdge[];
  groups?: CircuitDiagramGroup[];
  annotations?: Array<{
    target?: string;
    text: string;
  }>;
};
