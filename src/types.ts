export interface Annotation {
  type: "ring" | "arrow" | "box" | "underline";
  x: number;
  y: number;
  width?: number;
  height?: number;
  label?: string;
}

export interface VlmResponse {
  text: string;
  annotations: Annotation[];
}
