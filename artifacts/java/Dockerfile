# Raptor Game Server - Java Runtime
# Base image for running Java-based game servers (Minecraft, etc.)
# Registry: artifacts.lstan.eu/java:21
#
# Build for multiple platforms:
# docker buildx build --platform linux/amd64,linux/arm64 -t artifacts.lstan.eu/java:21 --push .

FROM eclipse-temurin:21-jre

LABEL maintainer="Raptor Panel"
LABEL org.opencontainers.image.source="https://github.com/raptor-panel/raptor"
LABEL org.opencontainers.image.description="Java 21 runtime for game servers"
LABEL org.opencontainers.image.title="artifacts.lstan.eu/java"
LABEL org.opencontainers.image.version="21"

# Install required packages
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    openssl \
    git \
    tar \
    sqlite3 \
    fontconfig \
    iproute2 \
    tzdata \
    locales \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Set up locale
RUN locale-gen en_US.UTF-8
ENV LANG=en_US.UTF-8
ENV LC_ALL=en_US.UTF-8

# Ensure Java is in PATH for all users/scripts
ENV JAVA_HOME=/opt/java/openjdk
ENV PATH="${JAVA_HOME}/bin:${PATH}"

# Create container user and directories
RUN useradd -m -d /home/container -s /bin/bash container
RUN mkdir -p /home/container && chown -R container:container /home/container

# Set working directory
WORKDIR /home/container

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Switch to non-root user
USER container

# Environment variables
ENV USER=container
ENV HOME=/home/container
ENV STARTUP="java -Xms128M -Xmx1024M -jar server.jar"

# Entrypoint
ENTRYPOINT ["/entrypoint.sh"]
