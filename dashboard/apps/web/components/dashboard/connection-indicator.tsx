"use client";

import { useConnectionStatus } from "@/hooks/use-connection-status";
import { cn } from "@/lib/utils";

export function ConnectionIndicator() {
  const { isOnline, isLoading } = useConnectionStatus();

  return (
    <div className="flex items-center gap-2 text-sm">
      <div
        className={cn(
          "h-2 w-2 rounded-full",
          isLoading
            ? "bg-gray-400 animate-pulse"
            : isOnline
              ? "bg-green-500"
              : "bg-red-500"
        )}
      />
      <span className="text-muted-foreground">
        {isLoading ? "Connecting..." : isOnline ? "Connected" : "Disconnected"}
      </span>
    </div>
  );
}
