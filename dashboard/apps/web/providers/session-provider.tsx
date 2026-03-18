"use client";

import { createContext, useContext, useMemo } from "react";
import type { Session } from "@/lib/auth";
import authClient from "@/lib/auth-client";

interface SessionContextValue {
  data: Session | null;
  isPending: boolean;
}

const SessionContext = createContext<SessionContextValue | undefined>(
  undefined
);

export function SessionProvider({
  initialSession,
  children,
}: {
  initialSession?: Session | null;
  children: React.ReactNode;
}) {
  const { data: clientSession, isPending } = authClient.useSession();
  const serverSession = initialSession ?? null;

  const value = useMemo<SessionContextValue>(() => {
    // Prefer server session as a fallback even after the client hook settles.
    // This avoids "split brain" when client session fetch is blocked/misconfigured
    // (e.g. trustedOrigins mismatch) but the server can still read the cookies.
    const effective = isPending
      ? (serverSession ?? clientSession)
      : (clientSession ?? serverSession);
    return { data: effective, isPending };
  }, [clientSession, serverSession, isPending]);

  return (
    <SessionContext.Provider value={value}>{children}</SessionContext.Provider>
  );
}

export function useSession(): SessionContextValue {
  const ctx = useContext(SessionContext);
  if (!ctx) {
    throw new Error("useSession must be used within a SessionProvider");
  }
  return ctx;
}
