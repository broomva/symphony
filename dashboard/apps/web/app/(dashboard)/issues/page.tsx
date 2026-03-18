"use client";

import { useSymphonyState } from "@/hooks/use-symphony-state";
import { IssuesTable } from "@/components/dashboard/issues-table";

export default function IssuesPage() {
  const { data: state, isLoading } = useSymphonyState();

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold">Issues</h1>
        <p className="text-muted-foreground">
          Active and retrying issue sessions
        </p>
      </div>

      {isLoading ? (
        <div className="h-[400px] rounded-lg border bg-card animate-pulse" />
      ) : (
        <IssuesTable
          running={state?.running ?? []}
          retrying={state?.retrying ?? []}
        />
      )}
    </div>
  );
}
