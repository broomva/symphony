"use client";

import { useTRPC } from "@/trpc/react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ConnectionIndicator } from "@/components/dashboard/connection-indicator";
import { toast } from "sonner";
import { Power, RefreshCw } from "lucide-react";

export default function ControlsPage() {
  const trpc = useTRPC();
  const queryClient = useQueryClient();

  const refreshMutation = useMutation(
    trpc.symphony.refresh.mutationOptions({
      onSuccess: (data) => {
        toast.success(
          data.coalesced
            ? "Refresh already queued (coalesced)"
            : "Refresh triggered successfully"
        );
        queryClient.invalidateQueries({ queryKey: [["symphony"]] });
      },
      onError: (error) => {
        toast.error(`Refresh failed: ${error.message}`);
      },
    })
  );

  const shutdownMutation = useMutation(
    trpc.symphony.shutdown.mutationOptions({
      onSuccess: () => {
        toast.success("Shutdown initiated");
      },
      onError: (error) => {
        toast.error(`Shutdown failed: ${error.message}`);
      },
    })
  );

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold">Controls</h1>
          <p className="text-muted-foreground">
            Manage the Symphony daemon
          </p>
        </div>
        <ConnectionIndicator />
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <RefreshCw className="h-5 w-5" />
              Trigger Poll
            </CardTitle>
            <CardDescription>
              Force an immediate tracker poll cycle. Safe to call at any time —
              duplicate requests are coalesced.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Button
              onClick={() => refreshMutation.mutate()}
              disabled={refreshMutation.isPending}
            >
              {refreshMutation.isPending ? "Triggering..." : "Trigger Poll"}
            </Button>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-destructive">
              <Power className="h-5 w-5" />
              Shutdown
            </CardTitle>
            <CardDescription>
              Initiate a graceful shutdown of the Symphony daemon. Running agents
              will be allowed to complete their current turn.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button variant="destructive">Shutdown Daemon</Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Confirm shutdown</AlertDialogTitle>
                  <AlertDialogDescription>
                    This will gracefully shut down the Symphony daemon. Running
                    agents will finish their current turn before stopping. You
                    will need to restart the daemon manually.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction
                    onClick={() => shutdownMutation.mutate()}
                    className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                  >
                    Confirm Shutdown
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
