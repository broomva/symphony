import { z } from "zod";
import {
  getDashboardSettings,
  upsertDashboardSettings,
} from "@/lib/db/queries";
import { createTRPCRouter, protectedProcedure } from "@/trpc/init";

export const settingsRouter = createTRPCRouter({
  get: protectedProcedure.query(async ({ ctx }) => {
    return getDashboardSettings({ userId: ctx.user.id });
  }),

  update: protectedProcedure
    .input(
      z.object({
        symphonyUrl: z.string().url().optional(),
        theme: z.string().optional(),
        refreshIntervalMs: z.number().int().min(1000).max(60000).optional(),
      })
    )
    .mutation(async ({ ctx, input }) => {
      await upsertDashboardSettings({
        userId: ctx.user.id,
        ...input,
      });
      return { success: true };
    }),
});
