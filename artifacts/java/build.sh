#!/bin/bash
# Build and push multi-architecture Java image
# Supports: linux/amd64 (Intel/AMD) and linux/arm64 (Apple Silicon, ARM servers)

set -e

IMAGE_NAME="${IMAGE_NAME:-artifacts.lstan.eu/java}"
IMAGE_TAG="${IMAGE_TAG:-21}"

echo "Building multi-arch image: ${IMAGE_NAME}:${IMAGE_TAG}"
echo "Platforms: linux/amd64, linux/arm64"

# Ensure buildx is available and create builder if needed
if ! docker buildx inspect multiarch-builder &>/dev/null; then
    echo "Creating buildx builder..."
    docker buildx create --name multiarch-builder --use
fi

docker buildx use multiarch-builder

# Build and push
docker buildx build \
    --platform linux/amd64,linux/arm64 \
    -t "${IMAGE_NAME}:${IMAGE_TAG}" \
    --push \
    .

echo "Done! Image pushed: ${IMAGE_NAME}:${IMAGE_TAG}"
