#!/bin/bash
set -e

IMAGE_NAME=${IMAGE_NAME:-"supertts"}
IMAGE_TAG=${IMAGE_TAG:-"latest"}
PLATFORMS=${PLATFORMS:-"linux/arm64"} # or linux/amd64
PUSH=${PUSH:-false}

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'


print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}


if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed or not in PATH"
    exit 1
fi

if ! docker buildx version &> /dev/null; then
    print_error "Docker buildx is not available. Please install buildx first."
    exit 1
fi


print_info "Checking buildx builder..."
if ! docker buildx inspect supertts-builder &> /dev/null; then
    print_info "Creating new buildx builder: supertts-builder"
    docker buildx create --name supertts-builder --use --bootstrap
else
    print_info "Using existing buildx builder: supertts-builder"
    docker buildx use supertts-builder
fi

# Build arguments
BUILD_ARGS=""
if [ "$PUSH" = "true" ]; then
    BUILD_ARGS="--push"
    print_info "Building and pushing to registry..."
else
    BUILD_ARGS="--load"
    print_info "Building locally..."
fi

# Build image
print_info "Building Docker image..."
print_info "Platforms: $PLATFORMS"
print_info "Image name: $IMAGE_NAME:$IMAGE_TAG"

# docker buildx build \
#     --platform $PLATFORMS \
#     --tag $IMAGE_NAME:$IMAGE_TAG \
#     --tag $IMAGE_NAME:$(git rev-parse --short HEAD 2>/dev/null || echo "dev") \
#     $BUILD_ARGS \
#     --pull=false \
#     .
docker build \
    --tag $IMAGE_NAME:$IMAGE_TAG \
    --tag $IMAGE_NAME:$(git rev-parse --short HEAD 2>/dev/null || echo "dev") \
    .

if [ "$PUSH" = "true" ]; then
    print_info "Image pushed successfully!"
else
    print_info "Image built successfully!"
    docker images | grep $IMAGE_NAME
fi


print_info ""
print_info "=== Usage ==="
print_info "Run container:"
echo "docker run -d -p 8080:8080 --name supertts $IMAGE_NAME:$IMAGE_TAG"
print_info ""
print_info "Or use docker-compose:"
echo "docker-compose --profile prod up supertts-prod"