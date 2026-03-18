"use client";

import { useSymphonyState } from "@/hooks/use-symphony-state";
import { useSymphonyMetrics } from "@/hooks/use-symphony-metrics";
import { StatCard } from "@/components/dashboard/stat-card";
import { ConnectionIndicator } from "@/components/dashboard/connection-indicator";
import { TokenChart } from "@/components/dashboard/token-chart";
import {
  Activity,
  Clock,
  Cpu,
  Hash,
  RefreshCw,
  Zap,
} from "lucide-react";
import { useRef } from "react";

export default function OverviewPage() {
  const { data: state, isLoading: stateLoading } = useSymphonyState();
  const { data: metrics } = useSymphonyMetrics();

  // Accumulate token history for chart
  const tokenHistory = useRef<
    { time: string; input_tokens: number; output_tokens: number }[]
  >([]);

  if (metrics) {
    const now = new Date().toLocaleTimeString();
    const last = tokenHistory.current[tokenHistory.current.length - 1];
    if (!last || last.time !== now) {
      tokenHistory.current = [
        ...tokenHistory.current.slice(-30),
        {
          time: now,
          input_tokens: metrics.totals.input_tokens,
          output_tokens: metrics.totals.output_tokens,
        },
      ];
    }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold">Overview</h1>
          <p className="text-muted-foreground">
            Symphony orchestration dashboard
          </p>
        </div>
        <ConnectionIndicator />
      </div>

      {stateLoading ? (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div
              key={i}
              className="h-[120px] rounded-lg border bg-card animate-pulse"
            />
          ))}
        </div>
      ) : (
        <>
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <StatCard
              title="Running"
              value={state?.counts.running ?? 0}
              description="Active agent sessions"
              icon={Activity}
            />
            <StatCard
              title="Retrying"
              value={state?.counts.retrying ?? 0}
              description="Issues in retry queue"
              icon={RefreshCw}
            />
            <StatCard
              title="Total Tokens"
              value={(state?.codex_totals.total_tokens ?? 0).toLocaleString()}
              description={`${(state?.codex_totals.input_tokens ?? 0).toLocaleString()} in / ${(state?.codex_totals.output_tokens ?? 0).toLocaleString()} out`}
              icon={Hash}
            />
            <StatCard
              title="Runtime"
              value={`${((state?.codex_totals.seconds_running ?? 0) / 60).toFixed(1)}m`}
              description="Total agent runtime"
              icon={Clock}
            />
          </div>

          {metrics && (
            <div className="grid gap-4 md:grid-cols-2">
              <StatCard
                title="Poll Interval"
                value={`${metrics.config.poll_interval_ms}ms`}
                description="Tracker polling frequency"
                icon={Zap}
              />
              <StatCard
                title="Max Concurrent"
                value={metrics.config.max_concurrent_agents}
                description="Maximum parallel agents"
                icon={Cpu}
              />
            </div>
          )}

          <TokenChart data={tokenHistory.current} />
        </>
      )}

      {state?.generated_at && (
        <p className="text-xs text-muted-foreground">
          Last updated: {new Date(state.generated_at).toLocaleString()}
        </p>
      )}
    </div>
  );
}
