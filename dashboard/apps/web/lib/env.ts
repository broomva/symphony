import { createEnv } from "@t3-oss/env-nextjs";
import { serverEnvSchema } from "./env-schema";

export const env = createEnv({
  server: serverEnvSchema,
  client: {},
  experimental__runtimeEnv: {},
  // Skip validation during Docker build (env vars are only available at runtime)
  skipValidation: !!process.env.SKIP_ENV_VALIDATION,
});
