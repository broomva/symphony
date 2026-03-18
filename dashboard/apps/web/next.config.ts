import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  typedRoutes: true,
  output: "standalone",
  serverExternalPackages: ["pino", "pino-pretty"],
};

export default nextConfig;
