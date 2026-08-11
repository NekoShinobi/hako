# Hako — common repo workflows.
#
#   just            list every recipe
#   just keygen     write a local .env with an ENCRYPTION_KEY
#   just dev        run the backend and the Vite dev server together
#   just ci         everything that should pass before a commit
#
# Values in .env are loaded automatically, so `just` and `docker compose` read
# the same local configuration.
#
# NOTE: `check` changed meaning. It used to be the full pre-commit gate
# (fmt --check, clippy, tests, svelte-check); it is now a fast type-check only.
# The gate is `just ci`.

set dotenv-load := true
set shell := ["bash", "-euo", "pipefail", "-c"]

# Local dev defaults. Anything exported in the shell, or set in .env, wins.
export DATABASE_URL := env_var_or_default("DATABASE_URL", "sqlite:hako.db")
export UPLOAD_DIR := env_var_or_default("UPLOAD_DIR", "./uploads")
export RUST_LOG := env_var_or_default("RUST_LOG", "info")

# Host identity, so files the dev containers write into the bind mount stay
# owned by you instead of root. Only `up-dev` needs these.
export DEV_UID := env_var_or_default("DEV_UID", `id -u`)
export DEV_GID := env_var_or_default("DEV_GID", `id -g`)

# Supply-chain cooldown: never adopt a release younger than this. Most malicious
# package releases are found and yanked within a few days, so waiting costs
# nothing and skips the window where you would be the one to find it.
# renovate.json's `minimumReleaseAge` MUST carry the same number — Renovate
# opens the automated PRs and cannot read this file.
DEPS_MIN_AGE_DAYS := "3"

# List available recipes.
[private]
default:
    @just --list

# ── Development ───────────────────────────────────────────────────────────────

# Install Rust and frontend dependencies exactly as locked.
[group('dev')]
setup:
    cargo fetch --locked
    cd ui && bun install --frozen-lockfile

# Run the backend and frontend development servers together.
[group('dev')]
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    # kill 0 signals the whole process group, so Vite's and bacon's own
    # children die too. `kill $(jobs -p)` leaves grandchildren behind.
    trap 'kill 0' EXIT
    just dev-api &
    just dev-ui &
    wait

# Run the backend with live reload.
[group('dev')]
dev-api:
    bacon --headless run

# Run the Vite dev server (proxies /config, /upload, /file, /img to the API).
[group('dev')]
dev-ui: node-modules
    cd ui && bun run dev

# bun install, but only when ui/node_modules is missing.
[private]
node-modules:
    @[ -d ui/node_modules ] || (cd ui && bun install --frozen-lockfile)

# Run the optimised binary, as in the README's from-source flow.
[group('dev')]
run *ARGS:
    cargo run --release {{ ARGS }}

# ── Build ─────────────────────────────────────────────────────────────────────

# Build every release artifact: the UI into ./static, plus the binary.
[group('build')]
build: build-ui build-api

# Release build of the Rust binary only.
[group('build')]
build-api:
    cargo build --release

# Build the SvelteKit UI and publish it to ./static, which Actix serves.
[group('build')]
build-ui: node-modules
    cd ui && bun run build
    rm -rf static
    cp -r ui/build static

# ── Quality ───────────────────────────────────────────────────────────────────

# Fast type-check — no formatting, no linting, no tests.
[group('checks')]
check: check-api check-ui

# Type-check the Rust crate.
[group('checks')]
check-api:
    cargo check --all-targets

# svelte-check type/diagnostics pass.
[group('checks')]
check-ui: node-modules
    cd ui && bun run check

# Rust test suite.
[group('checks')]
test *ARGS:
    cargo test {{ ARGS }}

# Run all backend and frontend linters.
[group('checks')]
lint: lint-api lint-ui

# Clippy over all targets, warnings promoted to errors.
[group('checks')]
lint-api:
    cargo clippy --all-targets --all-features -- -D warnings

# Prettier + ESLint.
[group('checks')]
lint-ui: node-modules
    cd ui && bun run lint

# Format Rust and frontend sources in place.
[group('checks')]
fmt: node-modules
    cargo fmt --all
    cd ui && bun run format

# Verify formatting without changing files.
[group('checks')]
fmt-check: node-modules
    cargo fmt --all -- --check
    cd ui && bunx prettier --check .

# Everything that should pass before a commit. This is what `check` used to be.
[group('checks')]
ci: fmt-check check lint test

# Run the configured pre-commit hooks across the repo.
[group('checks')]
precommit:
    pre-commit run --all-files

# ── Dependencies ──────────────────────────────────────────────────────────────

# Show available dependency updates without changing anything.
[group('deps')]
deps-outdated:
    cargo update --dry-run
    cd ui && bun outdated

# Refresh lockfiles within the declared semver ranges, honouring the cooldown.
[group('deps')]
deps-update:
    #!/usr/bin/env bash
    set -euo pipefail
    days="{{ DEPS_MIN_AGE_DAYS }}"
    seconds="$(( days * 24 * 60 * 60 ))"

    # -Zmin-publish-age is nightly-only, but this crate builds on stable — so
    # nightly is requested for this one command rather than pinned project-wide
    # in a rust-toolchain.toml, which would drag CI and Docker onto nightly too.
    if ! cargo +nightly -Z help 2>&1 | grep -q 'min-publish-age'; then
      echo "error: nightly cargo has no -Zmin-publish-age, so the ${days}-day cooldown cannot be enforced." >&2
      echo "Install it with: rustup toolchain install nightly" >&2
      exit 1
    fi

    # Deliberately NOT `bun update --latest`: that ignores the semver ranges in
    # package.json and walks every dependency to its newest major. Use
    # `deps-update-major` when major bumps are the actual intent.
    cargo +nightly update -Z min-publish-age --config "registry.global-min-publish-age = \"${days} days\""
    cd ui && bun update --minimum-release-age "$seconds"

# Bump frontend dependencies across major versions (review, then run `just ci`).
[group('deps')]
deps-update-major:
    #!/usr/bin/env bash
    set -euo pipefail
    # This ignores the semver ranges in package.json, so majors land in one go.
    seconds="$(( {{ DEPS_MIN_AGE_DAYS }} * 24 * 60 * 60 ))"
    cd ui && bun update --latest --minimum-release-age "$seconds"

# Scan dependencies for known vulnerabilities.
[group('deps')]
deps-audit:
    #!/usr/bin/env bash
    set -euo pipefail
    # RUSTSEC-2026-0235 (rkyv): the vulnerable feature is not enabled — see the
    # comment in .github/workflows/build.yml. Keep both in step.
    cargo audit --ignore RUSTSEC-2026-0235
    cd ui && bun audit

# Validate the Renovate policy that opens the automated update PRs.
[group('deps')]
deps-validate:
    bunx --package renovate renovate-config-validator --strict

# ── Docker ────────────────────────────────────────────────────────────────────

# Build the production image locally.
[group('docker')]
docker-build:
    docker build -t hako:local .

# Start the production stack in the foreground.
[group('docker')]
up:
    docker compose up --build

# Start the production stack in the background.
[group('docker')]
up-detach:
    docker compose up -d --build

# Start the full containerized dev stack (parity path — `just dev` is faster).
[group('docker')]
up-dev:
    docker compose -f compose.dev.yml up --build

# Stop every stack, keeping the /data and /uploads volumes.
[group('docker')]
down:
    -docker compose -f compose.dev.yml down
    -docker compose down

# Follow production container logs.
[group('docker')]
logs:
    docker compose logs -f

# Follow dev container logs.
[group('docker')]
logs-dev:
    docker compose -f compose.dev.yml logs -f

# Validate the Compose files without starting anything.
[group('docker')]
compose-check:
    docker compose -f compose.yml config --quiet
    docker compose -f compose.dev.yml config --quiet

# ── Housekeeping ──────────────────────────────────────────────────────────────

# Write a fresh ENCRYPTION_KEY to .env, never replacing an existing one.
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

# Delete the local database and uploads so the next start recreates them.
[group('db')]
db-reset:
    #!/usr/bin/env bash
    # Uploads are encrypted with ENCRYPTION_KEY and their metadata lives in the
    # database, so removing one without the other leaves undecryptable orphans.
    # Falls back to sudo when Docker-created files are root-owned.
    rm -f hako.db hako.db-shm hako.db-wal 2>/dev/null \
      || sudo rm -f hako.db hako.db-shm hako.db-wal
    rm -rf uploads/ 2>/dev/null || sudo rm -rf uploads/
    # .devdata/ is the same pair for the `up-dev` container.
    rm -rf .devdata/ 2>/dev/null || sudo rm -rf .devdata/
    echo "Database and uploads wiped (host and dev container). Both are recreated on next startup."

# ── Transitional aliases (remove once the new names are habit) ────────────────
# `check` is deliberately NOT aliased: it used to be the full gate and is now a
# fast type-check, so silently redirecting it would hide the change.

alias build-all := build
alias clippy := lint-api
alias ui-install := setup
alias ui-dev := dev-ui
alias ui-build := build-ui
alias ui-lint := lint-ui
alias ui-format := fmt
alias ui-check := check-ui
