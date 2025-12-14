# Multi-stage Dockerfile for superTTS

#
# Stage 1: Rust Builder
#
FROM rust:1.88.0-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    g++ \
    build-essential \
    libprotobuf-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

#
# Stage 2: Assets Builder
#
FROM alpine:3.16 AS assets

RUN apk add --no-cache git git-lfs

WORKDIR /app/assets

# Download assets from GitHub
# RUN git lfs install
# # Download ONNX models (using mirror for faster access https://hf-mirror.com/Supertone/supertonic) 
# RUN git clone https://huggingface.co/Supertone/supertonic .

## Copy assets from local directory
COPY assets ./

#
# Stage 3: Runtime
#
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 supertts && \
    mkdir -p /app/assets /app/results && \
    chown -R supertts:supertts /app

WORKDIR /app

COPY --from=builder /app/target/release/supertts /app/supertts
RUN chmod +x /app/supertts

COPY --from=assets /app/assets /app/assets
RUN chown -R supertts:supertts /app/assets

COPY example_config.json /app/config.json
RUN chown supertts:supertts /app/config.json

USER supertts

# Expose API port
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["./supertts", "--openai", "--config", "config.json"]