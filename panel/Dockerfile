# Build stage
FROM node:20-alpine AS builder

WORKDIR /app

# Copy package files first for better caching
COPY panel/package.json panel/package-lock.json* ./

# Install all dependencies (including devDependencies for build)
RUN npm ci 2>/dev/null || npm install

# Copy all source files
COPY panel/ ./

# Build with a placeholder that will be replaced at runtime
# Using a relative path so it works with any domain
ENV VITE_API_URL=""
RUN npm run build

# Prune dev dependencies, keep only production
RUN npm prune --production 2>/dev/null || true

# Runtime stage
FROM node:20-alpine

WORKDIR /app

# Copy the built application and production node_modules
COPY --from=builder /app/build ./build
COPY --from=builder /app/package.json ./
COPY --from=builder /app/node_modules ./node_modules

EXPOSE 3000

ENV NODE_ENV=production
ENV PORT=3000

# ORIGIN must be set at runtime to match your domain
# Example: ORIGIN=https://panel.example.com
CMD ["node", "build"]
