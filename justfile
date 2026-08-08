# Hako — common repo workflows.
#
#   just            list every recipe
#   just keygen     write a local .env with an ENCRYPTION_KEY
#   just dev        run the backend against ./hako.db and ./uploads
#
# Values in .env are loaded automatically, so `just` and `docker compose` read
# the same local configuration.

set dotenv-load := true
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Local dev defaults. Anything exported in the shell, or set in .env, wins.
export DATABASE_URL := env_var_or_default("DATABASE_URL", "sqlite:hako.db")
export UPLOAD_DIR := env_var_or_default("UPLOAD_DIR", "./uploads")
export RUST_LOG := env_var_or_default("RUST_LOG", "info")

[private]
default:
    @just --list --unsorted

# ── Backend ──────────────────────────────────────────────────────────────────

# Run the server in debug mode (fast rebuilds); extra args go to cargo run
[group('backend')]
dev *ARGS:
    cargo run {{ ARGS }}

# Run the optimised binary, as in the README's from-source flow
[group('backend')]
run *ARGS:
    cargo run --release {{ ARGS }}

# Release build of the Rust binary only
[group('backend')]
build:
    cargo build --release

# Frontend into ./static plus the release binary — a full from-source build
[group('backend')]
build-all: ui-build build

# ── Checks ───────────────────────────────────────────────────────────────────

# Format Rust sources in place
[group('checks')]
fmt:
    cargo fmt --all

# Clippy over all targets, warnings promoted to errors
[group('checks')]
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Rust test suite
[group('checks')]
test *ARGS:
    cargo test {{ ARGS }}

# Everything that should pass before a commit: Rust + UI
[group('checks')]
check: && ui-check
    cargo fmt --all -- --check
    just clippy
    cargo test

# Run the configured pre-commit hooks across the repo
[group('checks')]
precommit:
    pre-commit run --all-files

# ── Frontend ─────────────────────────────────────────────────────────────────

# npm ci, but only when ui/node_modules is missing
[private]
node-modules:
    @[ -d ui/node_modules ] || (cd ui && npm ci)

# Reinstall UI dependencies from the lockfile
[group('frontend')]
ui-install:
    cd ui && npm ci

# Vite dev server with hot reload (proxy the API to a running `just dev`)
[group('frontend')]
ui-dev: node-modules
    cd ui && npm run dev

# Build the SvelteKit UI and publish it to ./static, which Actix serves
[group('frontend')]
ui-build: node-modules
    cd ui && npm run build
    rm -rf static
    cp -r ui/build static

# Prettier + ESLint
[group('frontend')]
ui-lint: node-modules
    cd ui && npm run lint

# Rewrite UI sources with Prettier
[group('frontend')]
ui-format: node-modules
    cd ui && npm run format

# svelte-check type/diagnostics pass
[group('frontend')]
ui-check: node-modules
    cd ui && npm run check

# ── Docker ───────────────────────────────────────────────────────────────────

# Build the production image locally
[group('docker')]
docker-build:
    docker build -t hako:local .

# Start the compose stack in the background
[group('docker')]
up:
    docker compose up -d --build

# Stop the compose stack, keeping the /data and /uploads volumes
[group('docker')]
down:
    docker compose down

# Follow container logs
[group('docker')]
logs:
    docker compose logs -f

# ── Housekeeping ─────────────────────────────────────────────────────────────

# Write a fresh ENCRYPTION_KEY to .env, never replacing an existing one
[group('misc')]
keygen:
    @if [ -f .env ] && grep -q '^ENCRYPTION_KEY=' .env; then \
        echo ".env already defines ENCRYPTION_KEY — left untouched."; \
        echo "Rotating it would make existing uploads permanently undecryptable."; \
    else \
        echo "ENCRYPTION_KEY=$(openssl rand -base64 24)" >> .env; \
        echo "Wrote a new ENCRYPTION_KEY to .env"; \
    fi

# Drop build output. Leaves .env, hako.db and ./uploads alone.
[group('misc')]
clean:
    cargo clean
    rm -rf static ui/build ui/.svelte-kit
