"use client";

import { useSymphonyMetrics } from "@/hooks/use-symphony-metrics";
import { StatCard } from "@/components/dashboard/stat-card";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  BarChart3,
  Clock,
  Cpu,
  Hash,
  Layers,
  RefreshCw,
  Zap,
} from "lucide-react";

export default function MetricsPage() {
  const { data: metrics, isLoading } = useSymphonyMetrics();

  if (isLoading) {
    return (
      <div className="space-y-6">
        <h1 className="text-3xl font-bold">Metrics</h1>
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <div
              key={i}
              className="h-[120px] rounded-lg border bg-card animate-pulse"
            />
          ))}
        </div>
      </div>
    );
  }

  if (!metrics) return null;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold">Metrics</h1>
        <p className="text-muted-foreground">
          Token usage, concurrency, and configuration
        </p>
      </div>

      <div>
        <h2 className="text-xl font-semibold mb-4">Token Usage</h2>
        <div className="grid gap-4 md:grid-cols-3">
          <StatCard
            title="Input Tokens"
            value={metrics.totals.input_tokens.toLocaleString()}
            icon={Hash}
          />
          <StatCard
            title="Output Tokens"
            value={metrics.totals.output_tokens.toLocaleString()}
            icon={Hash}
          />
          <StatCard
            title="Total Tokens"
            value={metrics.totals.total_tokens.toLocaleString()}
            icon={BarChart3}
          />
        </div>
      </div>

      <div>
        <h2 className="text-xl font-semibold mb-4">Sessions</h2>
        <div className="grid gap-4 md:grid-cols-3">
          <StatCard
            title="Running"
            value={metrics.current.running_sessions}
            icon={Cpu}
          />
          <StatCard
            title="Retrying"
            value={metrics.current.retrying_sessions}
            icon={RefreshCw}
          />
          <StatCard
            title="Claimed Issues"
            value={metrics.current.claimed_issues}
            icon={Layers}
          />
        </div>
      </div>

      <div>
        <h2 className="text-xl font-semibold mb-4">Configuration</h2>
        <div className="grid gap-4 md:grid-cols-2">
          <StatCard
            title="Poll Interval"
            value={`${metrics.config.poll_interval_ms}ms`}
            icon={Zap}
          />
          <StatCard
            title="Max Concurrent Agents"
            value={metrics.config.max_concurrent_agents}
            icon={Cpu}
          />
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Clock className="h-4 w-4" />
            Runtime
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-2xl font-bold tabular-nums">
            {(metrics.totals.seconds_running / 60).toFixed(1)} minutes
          </p>
          <p className="text-sm text-muted-foreground">
            ({metrics.totals.seconds_running.toFixed(0)}s total agent runtime)
          </p>
        </CardContent>
      </Card>

      <p className="text-xs text-muted-foreground">
        Snapshot at {new Date(metrics.timestamp).toLocaleString()}
      </p>
    </div>
  );
}
