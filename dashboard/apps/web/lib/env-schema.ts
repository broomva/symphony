import { z } from "zod";

export const serverEnvSchema = {
  DATABASE_URL: z.string().min(1).describe("Postgres connection string"),
  AUTH_SECRET: z
    .string()
    .min(1)
    .describe("Secret for signing session tokens"),
  SYMPHONY_API_URL: z
    .string()
    .url()
    .default("http://localhost:8080")
    .describe("Symphony daemon HTTP API base URL"),
  SYMPHONY_API_TOKEN: z
    .string()
    .optional()
    .describe("Bearer token for Symphony API authentication"),
  APP_URL: z
    .url()
    .optional()
    .describe("App URL for non-Vercel deployments"),
};
