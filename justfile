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

# Format project.
[group("lint")]
fmt: update-readmes
    cargo +nightly fmt
    fd --type=file --hidden --extension=yml --extension=md --exec-batch npx -y prettier --write

# Check project.
[group("lint")]
check:
    cargo +nightly fmt -- --check
    fd --type=file --hidden --extension=yml --extension=md --exec-batch npx -y prettier --check

# Update READMEs from crate root documentation.
[group("lint")]
update-readmes:
    cd ./actix-cors && cargo rdme --force
    cd ./actix-identity && cargo rdme --force
    cd ./actix-session && cargo rdme --force
    fd README.md --exec-batch npx -y prettier --write

# Test workspace code.
[group("test")]
test:
    cargo {{ toolchain }} nextest run --workspace --all-features
    cargo {{ toolchain }} test --doc --workspace --all-features

# Downgrade dev-dependencies necessary to run MSRV checks/tests.
[private]
downgrade-for-msrv:

# Test workspace using MSRV.
[group("test")]
test-msrv: downgrade-for-msrv
    @just toolchain={{ msrv_rustup }} test

# Test workspace code and docs.
[group("test")]
test-all: test test-docs

# Test workspace and collect coverage info.
[private]
test-coverage:
    cargo {{ toolchain }} llvm-cov nextest --no-report --all-features
    cargo {{ toolchain }} llvm-cov --doc --no-report --all-features

# Test workspace and generate Codecov report.
test-coverage-codecov: test-coverage
    cargo {{ toolchain }} llvm-cov report --doctests --codecov --output-path=codecov.json

# Test workspace and generate LCOV report.
test-coverage-lcov: test-coverage
    cargo {{ toolchain }} llvm-cov report --doctests --lcov --output-path=lcov.info

# Test workspace docs.
[group("test")]
[group("docs")]
test-docs:
    # `tracing-actix-web` has mutually exclusive `opentelemetry_*` features (it supports multiple
    # OpenTelemetry versions for backward-compatibility), therefore `--all-features` cannot be used
    # for the entire workspace.
    cargo {{ toolchain }} test --doc --workspace --all-features --exclude tracing-actix-web --no-fail-fast -- --nocapture
    cargo {{ toolchain }} test --doc -p tracing-actix-web --no-fail-fast -- --nocapture
    cargo {{ toolchain }} test --doc -p tracing-actix-web --features uuid_v7 --no-fail-fast -- --nocapture
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_13
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_14
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_15
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_16
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_17
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_18
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_19
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_20
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_21
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_22
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_23
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_24
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_25
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_26
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_27
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_28
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_29
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_30
    cargo {{ toolchain }} check -p tracing-actix-web --all-targets --features opentelemetry_0_31

# CI: test `tracing-actix-web` with feature sets that cannot be covered by `--all-features`.
[group("test")]
ci-test-tracing-actix-web:
    cargo {{ toolchain }} test -p tracing-actix-web --lib --tests --examples --bins --no-fail-fast
    cargo {{ toolchain }} test -p tracing-actix-web --lib --tests --examples --bins --no-fail-fast --features uuid_v7
    cargo {{ toolchain }} test -p tracing-actix-web --lib --tests --examples --bins --no-fail-fast --features opentelemetry_0_13
    cargo {{ toolchain }} test -p tracing-actix-web --lib --tests --examples --bins --no-fail-fast --features opentelemetry_0_31

# Document crates in workspace.
[group("docs")]
doc *args: && doc-set-workspace-crates
    rm -f "$(cargo metadata --format-version=1 | jq -r '.target_directory')/doc/crates.js"
    # `tracing-actix-web` has mutually exclusive `opentelemetry_*` features, therefore `--all-features`
    # cannot be used for the entire workspace.
    RUSTDOCFLAGS="--cfg=docsrs -D warnings" cargo +nightly doc --workspace --no-deps --all-features --exclude tracing-actix-web {{ args }}
    RUSTDOCFLAGS="--cfg=docsrs -D warnings" cargo +nightly doc -p tracing-actix-web --no-deps --features "uuid_v7 opentelemetry_0_31" {{ args }}

[group("docs")]
[private]
doc-set-workspace-crates:
    #!/usr/bin/env bash
    (
        echo "window.ALL_CRATES = "
        cargo metadata --format-version=1 \
        | jq '[.packages[] | select(.source == null) | .targets | map(select(.doc) | .name)] | flatten'
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
