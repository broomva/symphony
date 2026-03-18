import { createEnv } from "@t3-oss/env-nextjs";
import { serverEnvSchema } from "./env-schema";

export const env = createEnv({
  server: serverEnvSchema,
  client: {},
  experimental__runtimeEnv: {},
});
