"use client";

import { useTRPC } from "@/trpc/react";
import { useQuery } from "@tanstack/react-query";

export function useConnectionStatus(refetchInterval = 5000) {
  const trpc = useTRPC();
  const { data, isLoading } = useQuery(
    trpc.symphony.health.queryOptions(undefined, { refetchInterval })
  );
  return {
    isOnline: data === true,
    isLoading,
  };
}
