FROM ghcr.io/nekoshinobi/rust-chef-sccache:main AS base
FROM base AS planner
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo chef prepare --recipe-path recipe.json

# Bun is the package manager. A build proxies nothing, so unlike the dev server
# it does not need to run under Node — see Dockerfile.ui-dev for why that
# distinction matters.
FROM oven/bun:1.3.14-alpine AS frontend-builder
WORKDIR /app/ui
COPY ui/package.json ui/bun.lock* ./
RUN bun install --frozen-lockfile
COPY ui/ .
RUN bun run build

FROM base AS builder
WORKDIR /app

COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo build --release

FROM ubuntu:26.04
WORKDIR /app

# Run as an unprivileged fixed uid. The process parses untrusted multipart input
# and writes attacker-supplied bytes to disk, so it has no business being root.
# Named volumes inherit this ownership; bind mounts do not — a host directory
# must be chown'd to 10001:10001 or the container cannot write to it.
RUN groupadd --system --gid 10001 hako && \
    useradd --system --uid 10001 --gid hako --no-create-home hako && \
    mkdir -p /data /uploads && \
    chown hako:hako /data /uploads && \
    chmod 750 /data /uploads

VOLUME ["/data", "/uploads"]

COPY --from=builder /app/target/release/hako hako
COPY --from=frontend-builder /app/ui/build /app/static

USER hako

EXPOSE 9090

CMD ["/app/hako"]
