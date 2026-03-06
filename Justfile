PLUGIN_VERSION := `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "pact_graphql_plugin") | .version'`
SUPPORTED_TARGETS := '["x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"]'
host-triple := `rustc -Vv | awk '/host/ {print $2}'`

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

install:
	@set -euo pipefail
	for cmd in gzip jq mktemp; do \
		if ! command -v "$$cmd" >/dev/null 2>&1; then \
			printf >&2 "'%s' is required for just install. Please install it and retry.\n" "$$cmd"; \
			exit 1; \
		fi; \
	done
	host_target="{{host-triple}}"
	if [ -z "$host_target" ]; then \
		printf >&2 "unable to detect host target via rustc\n"; \
		exit 1; \
	fi
	if ! rustup target list --installed | grep -Fxq "$host_target"; then \
		printf >&2 "rustup target '%s' is not installed. Run 'rustup target add %s' first.\n" "$host_target" "$host_target"; \
		exit 1; \
	fi
	label=$(just target-label target="$host_target")
	case "$host_target" in \
		*-pc-windows-msvc) \
			exe_suffix=.exe \
			;; \
		*) \
			exe_suffix= \
			;; \
		esac
	dist_dir="dist/$host_target"
	archive="$dist_dir/pact-graphql-plugin-$label$exe_suffix.gz"
	manifest="$dist_dir/pact-plugin.json"
	binary_name="pact-graphql-plugin$exe_suffix"
	if [ ! -f "$archive" ] || [ ! -f "$manifest" ]; then \
		printf "%s\n" "Host bundle missing; building $host_target"; \
		just bundle target="$host_target"; \
	fi
	tmp_dir=$(mktemp -d)
	cleanup() { rm -rf "$tmp_dir"; }
	trap cleanup EXIT INT TERM
	gzip -dc "$archive" > "$tmp_dir/$binary_name"
	cp "$manifest" "$tmp_dir/pact-plugin.json"
	case "$exe_suffix" in \
		.exe) \
			;; \
		*) \
			chmod +x "$tmp_dir/$binary_name"; \
			;; \
		esac
	install_root="$HOME/.pact/plugins/graphql-{{PLUGIN_VERSION}}"
	mkdir -p "$install_root"
	cp "$tmp_dir/$binary_name" "$install_root/$binary_name"
	cp "$tmp_dir/pact-plugin.json" "$install_root/pact-plugin.json"
	printf "Installed GraphQL plugin for %s at %s\n" "$host_target" "$install_root"
