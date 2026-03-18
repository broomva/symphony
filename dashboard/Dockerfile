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
COPY --from=deps /app/node_modules ./node_modules
COPY . .

# Build args for Next.js static generation
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

# Production runner
FROM base AS runner
WORKDIR /app
ENV NODE_ENV=production

# Copy built app and dependencies
COPY --from=builder /app ./

EXPOSE 3000
ENV PORT=3000
ENV HOSTNAME="0.0.0.0"

CMD ["bun", "run", "start"]
