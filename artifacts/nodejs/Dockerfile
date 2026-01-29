# Raptor Game Server - Node.js Runtime
# Base image for running Node.js-based game servers and applications
# Registry: artifacts.lstan.eu/nodejs:22
#
# Build for multiple platforms:
# docker buildx build --platform linux/amd64,linux/arm64 -t artifacts.lstan.eu/nodejs:22 --push .

FROM node:22-bookworm-slim

LABEL maintainer="Raptor Panel"
LABEL org.opencontainers.image.source="https://github.com/raptor-panel/raptor"
LABEL org.opencontainers.image.description="Node.js 22 runtime for game servers and applications"
LABEL org.opencontainers.image.title="artifacts.lstan.eu/nodejs"
LABEL org.opencontainers.image.version="22"

# Install required packages including bash and tini for proper signal handling
RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    curl \
    ca-certificates \
    openssl \
    git \
    tar \
    iproute2 \
    tzdata \
    jq \
    tini \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

# Set locale (pre-installed in base image)
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

# Ensure Node.js and npm are in PATH
ENV PATH="/usr/local/bin:${PATH}"

# Create container user (consistent with Java image and raptor daemon)
RUN useradd -m -d /home/container -s /bin/bash container
RUN mkdir -p /home/container && chown -R container:container /home/container

# Set working directory
WORKDIR /home/container

# Copy default package.json and index.js
COPY --chown=container:container default-package.json /home/container/package.json
COPY --chown=container:container default-index.js /home/container/index.js

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Switch to non-root user
USER container

# Environment variables
ENV USER=container
ENV HOME=/home/container
ENV STARTUP="node index.js"

# Entrypoint - use tini for proper signal handling and zombie reaping
# Note: We don't use -g (process group) flag to allow Docker's stop command
# to properly track the container state before signals are forwarded
ENTRYPOINT ["/usr/bin/tini", "--", "/entrypoint.sh"]
