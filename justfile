# depends on:
# - https://crates.io/crates/fd-find
# - https://crates.io/crates/cargo-check-external-types

_list:
    @just --list

toolchain := ""

msrv := ```
    cargo metadata --format-version=1 \
    | jq -r 'first(.packages[] | select(.source == null and .rust_version)) | .rust_version' \
    | sed -E 's/^1\.([0-9]{2})$/1\.\1\.0/'
```
msrv_rustup := "+" + msrv

# Run Clippy over workspace.
[group("lint")]
clippy:
    cargo {{ toolchain }} clippy --workspace --all-targets --all-features

# Format workspace.
[group("lint")]
fmt: update-readmes
    cargo +nightly fmt
    fd --hidden --extension=yml --extension=md --exec-batch npx -y prettier --write

# Update READMEs from crate root documentation.
[group("lint")]
update-readmes:
    cd ./actix-cors && cargo rdme --force
    cd ./actix-session && cargo rdme --force
    cd ./actix-identity && cargo rdme --force
    npx -y prettier --write $(fd README.md)

# Test workspace docs.
[group("test")]
[group("docs")]
test-docs:
    cargo {{ toolchain }} test --doc --workspace --all-features --no-fail-fast -- --nocapture

# Document crates in workspace.
[group("docs")]
doc *args: && doc-set-workspace-crates
    RUSTDOCFLAGS="--cfg=docsrs -Dwarnings" cargo +nightly doc --workspace --all-features {{ args }}

[private]
[group("docs")]
doc-set-workspace-crates:
    #!/usr/bin/env bash
    (
        echo "window.ALL_CRATES ="
        cargo metadata --format-version=1 | jq '[.packages[] | select(.source == null) | .name]'
        echo ";"
    ) > "$(cargo metadata --format-version=1 | jq -r '.target_directory')/doc/crates.js"

# Document crates in workspace and watch for changes.
[group("docs")]
doc-watch:
    @just doc --open
    cargo watch -- just doc

# Check for unintentional external type exposure on all crates in workspace.
[group("lint")]
check-external-types-all:
    #!/usr/bin/env bash
    set -euo pipefail
    exit=0
    for f in $(find . -mindepth 2 -maxdepth 2 -name Cargo.toml | grep -vE "\-codegen/|\-derive/|\-macros/"); do
        if ! just toolchain={{ toolchain }} check-external-types-manifest "$f"; then exit=1; fi
        echo
        echo
    done
    exit $exit

# Check for unintentional external type exposure on all crates in workspace.
[group("lint")]
check-external-types-all-table:
    #!/usr/bin/env bash
    set -euo pipefail
    for f in $(find . -mindepth 2 -maxdepth 2 -name Cargo.toml | grep -vE "\-codegen/|\-derive/|\-macros/"); do
        echo
        echo "Checking for $f"
        just toolchain={{ toolchain }} check-external-types-manifest "$f" --output-format=markdown-table
    done

# Check for unintentional external type exposure on a crate.
[group("lint")]
check-external-types-manifest manifest_path *extra_args="":
    cargo {{ toolchain }} check-external-types --manifest-path "{{ manifest_path }}" {{ extra_args }}
