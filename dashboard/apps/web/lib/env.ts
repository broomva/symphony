import { createEnv } from "@t3-oss/env-nextjs";
import { serverEnvSchema } from "./env-schema";

export const env = createEnv({
  server: serverEnvSchema,
  client: {},
  experimental__runtimeEnv: {},
  // Skip validation when required env vars are missing (Docker build, CI).
  // At runtime, missing vars will cause immediate errors on first use.
  skipValidation:
    !!process.env.SKIP_ENV_VALIDATION ||
    !process.env.DATABASE_URL,
});
