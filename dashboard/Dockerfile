# Symphony Dashboard — multi-stage Docker build
# For Railway / self-hosted deployment

FROM oven/bun:1.3 AS base
WORKDIR /app

# Install dependencies
FROM base AS deps
COPY package.json bun.lock ./
COPY apps/web/package.json ./apps/web/
COPY packages/symphony-client/package.json ./packages/symphony-client/
RUN bun install --frozen-lockfile

# Build the application
FROM base AS builder
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/apps/web/node_modules ./apps/web/node_modules
COPY . .

# Build args become env vars for Next.js build
ARG DATABASE_URL
ARG AUTH_SECRET
ARG SYMPHONY_API_URL=http://localhost:8080
ARG SYMPHONY_API_TOKEN
ARG APP_URL

ENV DATABASE_URL=${DATABASE_URL}
ENV AUTH_SECRET=${AUTH_SECRET}
ENV SYMPHONY_API_URL=${SYMPHONY_API_URL}
ENV SYMPHONY_API_TOKEN=${SYMPHONY_API_TOKEN}
ENV APP_URL=${APP_URL}

RUN bun run build

# Production image
FROM base AS runner
WORKDIR /app

ENV NODE_ENV=production

RUN addgroup --system --gid 1001 nodejs && \
    adduser --system --uid 1001 nextjs

COPY --from=builder /app/apps/web/.next/standalone ./
COPY --from=builder /app/apps/web/.next/static ./apps/web/.next/static
COPY --from=builder /app/apps/web/public ./apps/web/public

USER nextjs

EXPOSE 3000
ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

CMD ["node", "apps/web/server.js"]
