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
  sessionWarning?: string;
}

export interface BatchResult {
  snapshot: AppSnapshot;
  changed: string[];
  skipped: string[];
  errors: OperationError[];
}

export interface BatchCompletion {
  id: number;
  operation: "load" | "unload";
  result: BatchResult;
}

export interface BatchProgress {
  id: number;
  operation: "load" | "unload";
  completed: number;
  total: number;
}

export interface InitialState {
  snapshot: AppSnapshot;
  progress: BatchProgress | null;
  completion: BatchCompletion | null;
}

export interface ExitResult {
  exiting: boolean;
  errors: OperationError[];
}
