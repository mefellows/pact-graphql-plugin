PLUGIN_VERSION := `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "pact_graphql_plugin") | .version'`
SUPPORTED_TARGETS := '["x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"]'

version:
	@printf "%s\n" "{{PLUGIN_VERSION}}"

target-label target="":
	@case "{{target}}" in \
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
			printf >&2 "unsupported target '%s'. Supported targets: %s\n" "{{target}}" "$SUPPORTED_LIST"; \
			exit 1 \
			;; \
		esac; \
	printf "%s\n" "$label"

bundle target="":
	@set -euo pipefail
	if [ -z "{{target}}" ]; then \
		printf >&2 "target argument required (e.g. just bundle target=x86_64-apple-darwin)\n"; \
		exit 1; \
	fi
	target="{{target}}"
	label=$(just target-label target="$target")
	case "$target" in \
		*-pc-windows-msvc) \
			exe_suffix=.exe \
			;; \
		*) \
			exe_suffix= \
			;; \
	esac
	printf "Building pact-graphql-plugin for %s...\n" "$target"
	cargo build --release --target "$target"
	bin_path="target/$target/release/pact-graphql-plugin$exe_suffix"
	if [ ! -f "$bin_path" ]; then \
		printf >&2 "expected binary not found at %s\n" "$bin_path"; \
		exit 1; \
	fi
	dist_dir="dist/$target"
	mkdir -p "$dist_dir"
	stage_bin="$dist_dir/pact-graphql-plugin$exe_suffix"
	cp "$bin_path" "$stage_bin"
	manifest_path="$dist_dir/pact-plugin.json"
	jq --arg v "{{PLUGIN_VERSION}}" '.version = $v' pact-plugin.json > "$manifest_path"
	archive="$dist_dir/pact-graphql-plugin-$label$exe_suffix.gz"
	gzip -c "$stage_bin" > "$archive"
	if command -v shasum >/dev/null 2>&1; then \
		shasum -a 256 "$archive" > "$archive.sha256"; \
	elif command -v sha256sum >/dev/null 2>&1; then \
		sha256sum "$archive" > "$archive.sha256"; \
	else \
		printf >&2 "Neither shasum nor sha256sum is available; cannot write checksum\n"; \
		exit 1; \
	fi
	printf "Artifacts written to %s\n" "$dist_dir"
	printf "  binary: %s\n" "$stage_bin"
	printf "  manifest: %s\n" "$manifest_path"
	printf "  archive: %s\n" "$archive"
	printf "  checksum: %s.sha256\n" "$archive"

bundle-all:
	@set -euo pipefail
	printf "%s\n" "Bundling all supported targets..."
	targets=$(printf "%s" '{{SUPPORTED_TARGETS}}' | jq -r '.[]')
	IFS=$'\n'
	for target in $targets; do \
		printf "\n>> bundling %s\n" "$target"; \
		just bundle target="$target"; \
	done
	unset IFS
