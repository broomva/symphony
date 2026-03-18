/** Mirrors Rust StateSummary (symphony-observability server.rs) */
export interface StateSummary {
  generated_at: string;
  counts: Counts;
  running: RunningInfo[];
  retrying: RetryingInfo[];
  codex_totals: CodexTotalsInfo;
  rate_limits: unknown | null;
}

export interface Counts {
  running: number;
  retrying: number;
}

export interface RunningInfo {
  issue_id: string;
  identifier: string;
  session_id: string | null;
  state: string;
  started_at: string;
  turn_count: number;
  tokens: TokenInfo;
}

export interface RetryingInfo {
  issue_id: string;
  identifier: string;
  attempt: number;
  due_at_ms: number;
  error: string | null;
}

export interface TokenInfo {
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
}

export interface CodexTotalsInfo {
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  seconds_running: number;
}

/** GET /api/v1/metrics response */
export interface MetricsResponse {
  timestamp: string;
  totals: {
    input_tokens: number;
    output_tokens: number;
    total_tokens: number;
    seconds_running: number;
  };
  current: {
    running_sessions: number;
    retrying_sessions: number;
    claimed_issues: number;
  };
  config: {
    poll_interval_ms: number;
    max_concurrent_agents: number;
  };
}

/** GET /api/v1/{identifier} — running issue detail */
export interface IssueDetailRunning {
  identifier: string;
  state: string;
  session_id: string | null;
  started_at: string;
  turn_count: number;
  tokens: TokenInfo;
}

/** GET /api/v1/{identifier} — retrying issue detail */
export interface IssueDetailRetrying {
  identifier: string;
  status: "retrying";
  attempt: number;
  due_at_ms: number;
  error: string | null;
}

export type IssueDetail = IssueDetailRunning | IssueDetailRetrying;

/** GET /api/v1/workspaces response item */
export interface WorkspaceEntry {
  name: string;
  status: "running" | "retrying";
}

/** POST /api/v1/refresh response */
export interface RefreshResponse {
  queued: boolean;
  coalesced: boolean;
  requested_at: string;
  operations: string[];
}

/** POST /api/v1/shutdown response */
export interface ShutdownResponse {
  shutdown: boolean;
  requested_at: string;
}

/** Error envelope from API */
export interface ErrorEnvelope {
  error: {
    code: string;
    message: string;
  };
}
