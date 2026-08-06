export type FontFormat = "TTF" | "OTF" | "TTC";

export interface FontFace {
  family: string;
  style: string;
}

export interface FontEntry {
  canonicalPath: string;
  fileName: string;
  format: FontFormat;
  faces: FontFace[];
  loadedAt: string;
}

export interface OperationError {
  path: string;
  code: string;
  message: string;
}

export interface AppSnapshot {
  fonts: FontEntry[];
  restoreCandidates: string[];
  exitErrors: OperationError[];
}

export interface BatchResult {
  snapshot: AppSnapshot;
  changed: string[];
  skipped: string[];
  errors: OperationError[];
}

export interface BatchCompletion {
  operation: "load" | "unload";
  result: BatchResult;
}

export interface BatchProgress {
  operation: "load" | "unload";
  completed: number;
  total: number;
}

export interface ExitResult {
  exiting: boolean;
  errors: OperationError[];
}
