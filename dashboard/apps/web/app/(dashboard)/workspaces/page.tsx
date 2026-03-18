"use client";

import { useTRPC } from "@/trpc/react";
import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBadge } from "@/components/dashboard/status-badge";
import { FolderOpen } from "lucide-react";

export default function WorkspacesPage() {
  const trpc = useTRPC();
  const { data: workspaces, isLoading } = useQuery(
    trpc.symphony.getWorkspaces.queryOptions(undefined, { refetchInterval: 5000 })
  );

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold">Workspaces</h1>
        <p className="text-muted-foreground">
          Active workspace directories
        </p>
      </div>

      {isLoading ? (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 3 }).map((_, i) => (
            <div
              key={i}
              className="h-[100px] rounded-lg border bg-card animate-pulse"
            />
          ))}
        </div>
      ) : workspaces && workspaces.length > 0 ? (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {workspaces.map((ws) => (
            <Card key={ws.name}>
              <CardHeader className="flex flex-row items-center gap-3 pb-2">
                <FolderOpen className="h-5 w-5 text-muted-foreground" />
                <CardTitle className="text-base">{ws.name}</CardTitle>
              </CardHeader>
              <CardContent>
                <StatusBadge status={ws.status} />
              </CardContent>
            </Card>
          ))}
        </div>
      ) : (
        <Card>
          <CardContent className="pt-6 text-center text-muted-foreground">
            No active workspaces
          </CardContent>
        </Card>
      )}
    </div>
  );
}
