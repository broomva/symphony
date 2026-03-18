import type {
  ErrorEnvelope,
  IssueDetail,
  MetricsResponse,
  RefreshResponse,
  ShutdownResponse,
  StateSummary,
  WorkspaceEntry,
} from "./types";

export class SymphonyClient {
  private baseUrl: string;
  private token?: string;

  constructor(baseUrl: string, token?: string) {
    // Remove trailing slash
    this.baseUrl = baseUrl.replace(/\/+$/, "");
    this.token = token;
  }

  private async fetch<T>(path: string, init?: RequestInit): Promise<T> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      ...(init?.headers as Record<string, string>),
    };

    if (this.token) {
      headers["Authorization"] = `Bearer ${this.token}`;
    }

    const res = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers,
    });

    if (!res.ok) {
      let errorMessage = `HTTP ${res.status}`;
      try {
        const body = (await res.json()) as ErrorEnvelope;
        errorMessage = body.error?.message ?? errorMessage;
      } catch {
        // ignore parse errors
      }
      throw new Error(errorMessage);
    }

    return res.json() as Promise<T>;
  }

  async getState(): Promise<StateSummary> {
    return this.fetch<StateSummary>("/api/v1/state");
  }

  async getMetrics(): Promise<MetricsResponse> {
    return this.fetch<MetricsResponse>("/api/v1/metrics");
  }

  async getWorkspaces(): Promise<WorkspaceEntry[]> {
    return this.fetch<WorkspaceEntry[]>("/api/v1/workspaces");
  }

  async getIssue(identifier: string): Promise<IssueDetail> {
    return this.fetch<IssueDetail>(
      `/api/v1/${encodeURIComponent(identifier)}`
    );
  }

  async triggerRefresh(): Promise<RefreshResponse> {
    return this.fetch<RefreshResponse>("/api/v1/refresh", { method: "POST" });
  }

  async triggerShutdown(): Promise<ShutdownResponse> {
    return this.fetch<ShutdownResponse>("/api/v1/shutdown", { method: "POST" });
  }

  async checkHealth(): Promise<boolean> {
    try {
      const res = await fetch(`${this.baseUrl}/readyz`);
      return res.ok;
    } catch {
      return false;
    }
  }
}
