# Symphony Dashboard — Docker build for Railway
FROM oven/bun:1.3 AS base
WORKDIR /app

# Install dependencies
FROM base AS deps
COPY package.json bun.lock ./
COPY apps/web/package.json ./apps/web/
COPY packages/symphony-client/package.json ./packages/symphony-client/
RUN bun install

# Build the application
FROM base AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .

# Skip env validation during build — env vars only available at runtime
ENV SKIP_ENV_VALIDATION=1

RUN bun run build

# Production runner
FROM base AS runner
WORKDIR /app
ENV NODE_ENV=production

COPY --from=builder /app ./

EXPOSE 3000
ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

CMD ["bun", "run", "start"]
