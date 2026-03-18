"use client";

import { useTRPC } from "@/trpc/react";
import { useQuery } from "@tanstack/react-query";
import { useParams } from "next/navigation";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBadge } from "@/components/dashboard/status-badge";
import { ArrowLeft } from "lucide-react";
import Link from "next/link";

export default function IssueDetailPage() {
  const params = useParams<{ identifier: string }>();
  const trpc = useTRPC();
  const { data, isLoading, error } = useQuery(
    trpc.symphony.getIssue.queryOptions(
      { identifier: params.identifier },
      { refetchInterval: 5000 }
    )
  );

  if (isLoading) {
    return (
      <div className="space-y-6">
        <div className="h-8 w-48 rounded bg-muted animate-pulse" />
        <div className="h-[300px] rounded-lg border bg-card animate-pulse" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-4">
        <Link
          href="/issues"
          className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
        >
          <ArrowLeft className="h-4 w-4" /> Back to issues
        </Link>
        <Card>
          <CardContent className="pt-6">
            <p className="text-destructive">
              Error loading issue: {error.message}
            </p>
          </CardContent>
        </Card>
      </div>
    );
  }

  if (!data) return null;

  const isRetrying = "status" in data && (data as { status: string }).status === "retrying";
  // Type-narrow for the running case
  const running = !isRetrying ? (data as import("@symphony/client").IssueDetailRunning) : null;
  const retrying = isRetrying ? (data as import("@symphony/client").IssueDetailRetrying) : null;

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Link
          href="/issues"
          className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
        >
          <ArrowLeft className="h-4 w-4" /> Back
        </Link>
        <h1 className="text-3xl font-bold">{params.identifier}</h1>
        <StatusBadge status={isRetrying ? "retrying" : "running"} />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Details</CardTitle>
        </CardHeader>
        <CardContent>
          <dl className="grid grid-cols-2 gap-4">
            <div>
              <dt className="text-sm font-medium text-muted-foreground">
                Identifier
              </dt>
              <dd className="text-sm">{data.identifier}</dd>
            </div>
            {retrying ? (
              <>
                <div>
                  <dt className="text-sm font-medium text-muted-foreground">
                    Attempt
                  </dt>
                  <dd className="text-sm">{retrying.attempt}</dd>
                </div>
                <div>
                  <dt className="text-sm font-medium text-muted-foreground">
                    Due At
                  </dt>
                  <dd className="text-sm">
                    {new Date(retrying.due_at_ms).toLocaleString()}
                  </dd>
                </div>
                {retrying.error && (
                  <div className="col-span-2">
                    <dt className="text-sm font-medium text-muted-foreground">
                      Error
                    </dt>
                    <dd className="text-sm text-destructive">{retrying.error}</dd>
                  </div>
                )}
              </>
            ) : running ? (
              <>
                <div>
                  <dt className="text-sm font-medium text-muted-foreground">
                    State
                  </dt>
                  <dd className="text-sm">{running.state}</dd>
                </div>
                <div>
                  <dt className="text-sm font-medium text-muted-foreground">
                    Session ID
                  </dt>
                  <dd className="text-sm font-mono">
                    {running.session_id ?? "—"}
                  </dd>
                </div>
                <div>
                  <dt className="text-sm font-medium text-muted-foreground">
                    Started At
                  </dt>
                  <dd className="text-sm">
                    {new Date(running.started_at).toLocaleString()}
                  </dd>
                </div>
                <div>
                  <dt className="text-sm font-medium text-muted-foreground">
                    Turn Count
                  </dt>
                  <dd className="text-sm">{running.turn_count}</dd>
                </div>
                <div>
                  <dt className="text-sm font-medium text-muted-foreground">
                    Tokens
                  </dt>
                  <dd className="text-sm tabular-nums">
                    {running.tokens.input_tokens.toLocaleString()} in /{" "}
                    {running.tokens.output_tokens.toLocaleString()} out /{" "}
                    {running.tokens.total_tokens.toLocaleString()} total
                  </dd>
                </div>
              </>
            ) : null}
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}
