PLUGIN_VERSION := `cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "pact_graphql_plugin") | .version'`
SUPPORTED_TARGETS := '["x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"]'

version:
	@printf "%s\n" "{{PLUGIN_VERSION}}"

target-label target="":
	case "{{target}}" in
	x86_64-apple-darwin)
	label=macos-x86_64
	;;
	aarch64-apple-darwin)
	label=macos-aarch64
	;;
	x86_64-unknown-linux-gnu)
	label=linux-x86_64
	;;
	aarch64-unknown-linux-gnu)
	label=linux-aarch64
	;;
	x86_64-pc-windows-msvc)
	label=windows-x86_64
	;;
	aarch64-pc-windows-msvc)
	label=windows-aarch64
	;;
	*)
	SUPPORTED_LIST=$$(printf "%s" '{{SUPPORTED_TARGETS}}' | jq -r '.[]' | paste -sd "," - | sed 's/,/, /g')
	printf >&2 "unsupported target '%s'. Supported targets: %s\n" "{{target}}" "$$SUPPORTED_LIST"
	exit 1
	;;
	esac
	printf "%s\n" "$label"
