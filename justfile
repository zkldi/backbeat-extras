[private]
interactive:
	-@just --choose

# Run the native desktop app.
pkg-gui:
	bun run --cwd rust/backbeat_packager_app tauri dev

# Build desktop bundles.
build:
	bun run --cwd rust/backbeat_packager_app tauri build -- --locked

# Build a release DMG for this Mac's architecture.
[macos]
build-dmg:
	bun install --frozen-lockfile
	bun run --cwd rust/backbeat_packager_app tauri build --ci --bundles dmg -- --locked

check:
	bun run --filter '*' typecheck
	bun run --filter '*' lint
	bun run --filter '*' build
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings

test FILTER="*":
	#!/bin/bash
	set -euo pipefail

	source .scripts/multi_evaluator.sh

	evaluate "just test-typescript '{{FILTER}}'"
	evaluate "just test-rust '{{FILTER}}'"

	post_evaluate

test-typescript FILTER="*":
	#!/bin/bash
	set -euo pipefail
	set +e
	output="$(bun run --filter '{{FILTER}}' test -- --reporter=default 2>&1)"
	status=$?
	set -e
	if [ "$status" -ne 0 ] && [[ "$output" == *"No packages matched the filter"* || "$output" == *"No workspace packages matched the filter"* ]]; then
		echo "No workspace packages define test yet."
		exit 0
	fi
	printf '%s\n' "$output"
	exit "$status"

test-rust FILTER="*":
	#!/bin/bash
	set -euo pipefail

	if [[ "{{FILTER}}" == *"|"* ]]; then
		echo "\`|\` doesn't work in the regex here for tests, sorry" >&2
		exit 1
	fi

	if [ "{{FILTER}}" = "*" ]; then
		cargo nextest run --locked --cargo-quiet
	else
		cargo nextest run --locked --cargo-quiet '{{FILTER}}'
	fi

test-rust-package PACKAGE FILTER="*":
	#!/bin/bash
	set -euo pipefail

	if [ "{{FILTER}}" = "*" ]; then
		cargo nextest run --locked --cargo-quiet -p "{{PACKAGE}}"
	else
		cargo nextest run --locked --cargo-quiet -p "{{PACKAGE}}" '{{FILTER}}'
	fi

cli-docker-build:
	#!/bin/bash
	set -euo pipefail
	DOCKER_BUILDKIT=1 docker build -t backbeat-cli:latest \
		-f docker/Dockerfile.backbeat-cli \
		--build-arg "OCI_CREATED=$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
		--build-arg "OCI_REVISION=$(git rev-parse HEAD)" \
		.

cli-flamegraph *args:
	CARGO_HOME=$HOME/.cargo PATH=$PATH \
		cargo flamegraph --profile dev-release -p bkb --bin bkb -- {{args}}
