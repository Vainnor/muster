# syntax=docker/dockerfile:1
# Build from the repository root: docker build -f web/Dockerfile -t muster-web .
#
# Base images are pinned by digest; Dependabot (docker ecosystem) proposes updates.

FROM rust:1.99.0-slim-bookworm@sha256:2c3a22f0a5533ea2dd5a16627bc841228151faa2d4de2644ac9987e4a2f1f2fa AS build
WORKDIR /src
# The asset bundler must match the topcoat version in web/Cargo.toml.
RUN cargo install topcoat-cli --version 0.10.0 --locked
COPY . .
# Builds the release binary and bundles its assets (Tailwind CSS, favicon) from that build.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target-cache \
    CARGO_TARGET_DIR=/src/target-cache topcoat asset bundle --release -p muster-web --out /out/assets \
 && cp /src/target-cache/release/muster-web /out/muster-web

# glibc runtime, no shell, no package manager, runs as uid 65532.
FROM gcr.io/distroless/cc-debian12:nonroot@sha256:9dac0a79194e45a7da0158a9c6da57b217585af0786db3845d1f0ec1a0dd182f
COPY --from=build /out/muster-web /app/muster-web
COPY --from=build /out/assets /app/assets
ENV HOST=0.0.0.0 \
    PORT=8080 \
    MUSTER_WEB_ASSETS=/app/assets
USER nonroot:nonroot
EXPOSE 8080
ENTRYPOINT ["/app/muster-web"]
