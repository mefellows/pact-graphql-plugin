# The recipes below are bash: they use `set -o pipefail`, which `just`'s default `sh` does not
# support where /bin/sh is dash (Debian/Ubuntu, and so every Linux CI runner). macOS gets away
# with it because its /bin/sh is bash in POSIX mode, which is why this only showed up in CI.
set shell := ["bash", "-uc"]

# `just` defaults to cmd.exe on Windows regardless of `shell`, which would fail on every recipe
# here, so point it at bash too (Git Bash / WSL, as the README requires).
set windows-shell := ["bash", "-uc"]

PLUGIN_VERSION := `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "pact_graphql_plugin") | .version'`
SUPPORTED_TARGETS := '["x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"]'
host-triple := `rustc -Vv | awk '/host/ {print $2}'`

version:
	@printf "%s\n" "{{PLUGIN_VERSION}}"

# Checks the plugin crate, the npm package and the DSL's minimum plugin version agree.
check-versions:
	@./scripts/check-versions.sh

target-label target="":
	@target_value="{{target}}"; \
	target_value="${target_value#target=}"; \
	case "$target_value" in \
		x86_64-apple-darwin) \
			label=macos-x86_64 \
			;; \
		aarch64-apple-darwin) \
			label=macos-aarch64 \
			;; \
		x86_64-unknown-linux-gnu) \
			label=linux-x86_64 \
			;; \
		aarch64-unknown-linux-gnu) \
			label=linux-aarch64 \
			;; \
		x86_64-pc-windows-msvc) \
			label=windows-x86_64 \
			;; \
		aarch64-pc-windows-msvc) \
			label=windows-aarch64 \
			;; \
		*) \
			SUPPORTED_LIST=$(printf "%s" '{{SUPPORTED_TARGETS}}' | jq -r '.[]' | paste -sd "," - | sed 's/,/, /g'); \
			printf >&2 "unsupported target '%s'. Supported targets: %s\n" "$target_value" "$SUPPORTED_LIST"; \
			exit 1 \
			;; \
		esac; \
	printf "%s\n" "$label"

bundle target="":
	@set -euo pipefail; \
	target_value="{{target}}"; \
	target_value="${target_value#target=}"; \
	if [ -z "$target_value" ]; then \
		printf >&2 "target argument required (e.g. just bundle x86_64-apple-darwin)\n"; \
		exit 1; \
	fi; \
	label=$(just target-label "$target_value"); \
	exe_suffix=; \
	case "$target_value" in \
		*-pc-windows-msvc) exe_suffix=.exe ;; \
		*) exe_suffix= ;; \
	esac; \
	printf "Building pact-graphql-plugin for %s...\n" "$target_value"; \
	cargo build --release --target "$target_value"; \
	bin_path="target/$target_value/release/pact_graphql_plugin$exe_suffix"; \
	if [ ! -f "$bin_path" ]; then \
		printf >&2 "expected binary not found at %s\n" "$bin_path"; \
		exit 1; \
	fi; \
	dist_dir="dist/$target_value"; \
	mkdir -p "$dist_dir"; \
	stage_bin="$dist_dir/pact-graphql-plugin$exe_suffix"; \
	cp "$bin_path" "$stage_bin"; \
	manifest_path="$dist_dir/pact-plugin.json"; \
	jq --arg v "{{PLUGIN_VERSION}}" '.version = $v' pact-plugin.json > "$manifest_path"; \
	archive="$dist_dir/pact-graphql-plugin-$label$exe_suffix.gz"; \
	gzip -c "$stage_bin" > "$archive"; \
	if command -v shasum >/dev/null 2>&1; then \
		shasum -a 256 "$archive" > "$archive.sha256"; \
	elif command -v sha256sum >/dev/null 2>&1; then \
		sha256sum "$archive" > "$archive.sha256"; \
	else \
		printf >&2 "Neither shasum nor sha256sum is available; cannot write checksum\n"; \
		exit 1; \
	fi; \
	printf "Artifacts written to %s\n" "$dist_dir"; \
	printf "  binary: %s\n" "$stage_bin"; \
	printf "  manifest: %s\n" "$manifest_path"; \
	printf "  archive: %s\n" "$archive"; \
	printf "  checksum: %s.sha256\n" "$archive"

bundle-all:
	@set -euo pipefail
	printf "%s\n" "Bundling all supported targets..."
	targets=$(printf "%s" '{{SUPPORTED_TARGETS}}' | jq -r '.[]')
	IFS=$'\n'
	for target in $targets; do \
		printf "\n>> bundling %s\n" "$target"; \
		just bundle "$target"; \
	done
	unset IFS

install:
	@set -euo pipefail; \
	for cmd in gzip jq mktemp; do \
		if ! command -v "$cmd" >/dev/null 2>&1; then \
			printf >&2 "'%s' is required for just install. Please install it and retry.\n" "$cmd"; \
			exit 1; \
		fi; \
	done; \
	host_target="{{host-triple}}"; \
	if [ -z "$host_target" ]; then \
		printf >&2 "unable to detect host target via rustc\n"; \
		exit 1; \
	fi; \
	if ! rustup target list --installed | grep -Fxq "$host_target"; then \
		printf >&2 "rustup target '%s' is not installed. Run 'rustup target add %s' first.\n" "$host_target" "$host_target"; \
		exit 1; \
	fi; \
	label=$(just target-label "$host_target"); \
	exe_suffix=; \
	case "$host_target" in \
		*-pc-windows-msvc) exe_suffix=.exe ;; \
		*) exe_suffix= ;; \
	esac; \
	dist_dir="dist/$host_target"; \
	archive="$dist_dir/pact-graphql-plugin-$label$exe_suffix.gz"; \
	manifest="$dist_dir/pact-plugin.json"; \
	binary_name="pact-graphql-plugin$exe_suffix"; \
	printf "%s\n" "Building host bundle for $host_target"; \
	just bundle "$host_target"; \
	tmp_dir=$(mktemp -d); \
	cleanup() { rm -rf "$tmp_dir"; }; \
	trap cleanup EXIT INT TERM; \
	gzip -dc "$archive" > "$tmp_dir/$binary_name"; \
	cp "$manifest" "$tmp_dir/pact-plugin.json"; \
	case "$exe_suffix" in \
		.exe) ;; \
		*) chmod +x "$tmp_dir/$binary_name" ;; \
	esac; \
	install_root="$HOME/.pact/plugins/graphql-{{PLUGIN_VERSION}}"; \
	mkdir -p "$install_root"; \
	rm -f "$install_root/$binary_name"; \
	cp "$tmp_dir/$binary_name" "$install_root/$binary_name"; \
	cp "$tmp_dir/pact-plugin.json" "$install_root/pact-plugin.json"; \
	if command -v codesign >/dev/null 2>&1; then \
		codesign --force --sign - "$install_root/$binary_name" >/dev/null 2>&1 || true; \
	fi; \
	printf "Installed GraphQL plugin for %s at %s\n" "$host_target" "$install_root"
