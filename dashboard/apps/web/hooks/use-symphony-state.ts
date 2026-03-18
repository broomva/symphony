"use client";

import { useTRPC } from "@/trpc/react";
import { useQuery } from "@tanstack/react-query";

export function useSymphonyState(refetchInterval = 5000) {
  const trpc = useTRPC();
  return useQuery(
    trpc.symphony.getState.queryOptions(undefined, { refetchInterval })
  );
}
