# Build stage
FROM node:20-alpine AS builder

WORKDIR /app

# Copy package files first for better caching
COPY panel/package.json panel/package-lock.json* ./

# Install all dependencies (including devDependencies for build)
RUN npm ci 2>/dev/null || npm install

# Copy all source files
COPY panel/ ./

# Build the application
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

# Create entrypoint script that injects runtime config
RUN echo '#!/bin/sh' > /entrypoint.sh && \
    echo 'CONFIG_FILE="/app/build/client/config.js"' >> /entrypoint.sh && \
    echo 'echo "window.__CONFIG__ = { API_URL: \"${API_URL:-}\" };" > "$CONFIG_FILE"' >> /entrypoint.sh && \
    echo 'echo "Runtime config: API_URL=${API_URL:-<empty>}"' >> /entrypoint.sh && \
    echo 'exec node build' >> /entrypoint.sh && \
    chmod +x /entrypoint.sh

EXPOSE 3000

ENV NODE_ENV=production
ENV PORT=3000

# API_URL can be set at runtime
# Example: docker run -e API_URL=https://api.example.com ...
CMD ["/entrypoint.sh"]
