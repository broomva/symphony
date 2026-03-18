import { SymphonyClient } from "@symphony/client";
import { z } from "zod";
import { createTRPCRouter, protectedProcedure, publicProcedure } from "../init";

function getSymphonyClient() {
  const baseUrl = process.env.SYMPHONY_API_URL ?? "http://localhost:8080";
  const token = process.env.SYMPHONY_API_TOKEN;
  return new SymphonyClient(baseUrl, token);
}

export const symphonyRouter = createTRPCRouter({
  getState: publicProcedure.query(async () => {
    const client = getSymphonyClient();
    return client.getState();
  }),

  getMetrics: publicProcedure.query(async () => {
    const client = getSymphonyClient();
    return client.getMetrics();
  }),

  getWorkspaces: publicProcedure.query(async () => {
    const client = getSymphonyClient();
    return client.getWorkspaces();
  }),

  getIssue: publicProcedure
    .input(z.object({ identifier: z.string() }))
    .query(async ({ input }) => {
      const client = getSymphonyClient();
      return client.getIssue(input.identifier);
    }),

  health: publicProcedure.query(async () => {
    const client = getSymphonyClient();
    return client.checkHealth();
  }),

  refresh: protectedProcedure.mutation(async () => {
    const client = getSymphonyClient();
    return client.triggerRefresh();
  }),

  shutdown: protectedProcedure.mutation(async () => {
    const client = getSymphonyClient();
    return client.triggerShutdown();
  }),
});
