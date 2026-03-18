"use client";

import { useTRPC } from "@/trpc/react";
import { useQuery } from "@tanstack/react-query";

export function useSymphonyMetrics(refetchInterval = 5000) {
  const trpc = useTRPC();
  return useQuery(
    trpc.symphony.getMetrics.queryOptions(undefined, { refetchInterval })
  );
}
