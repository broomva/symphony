import { createCallerFactory, createTRPCRouter } from "@/trpc/init";
import { settingsRouter } from "./settings.router";
import { symphonyRouter } from "./symphony.router";

export const appRouter = createTRPCRouter({
  symphony: symphonyRouter,
  settings: settingsRouter,
});

export type AppRouter = typeof appRouter;

export const createCaller = createCallerFactory(appRouter);
