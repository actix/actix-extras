# depends on:
# - https://crates.io/crates/fd-find
# - https://crates.io/crates/cargo-check-external-types
_list:
    @just --list

# Format workspace.
fmt: update-readmes
    cargo +nightly fmt
    npx -y prettier --write $(fd --hidden --extension=yml --extension=md)

# Update READMEs from crate root documentation.
update-readmes:
    cd ./actix-cors && cargo rdme --force
    cd ./actix-session && cargo rdme --force
    cd ./actix-identity && cargo rdme --force
    npx -y prettier --write $(fd README.md)

# Document crates in workspace.
doc:
    RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc --no-deps --workspace --all-features

# Document crates in workspace and watch for changes.
doc-watch:
    RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc --no-deps --workspace --all-features --open
    cargo watch -- RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc --no-deps --workspace --all-features

# Check for unintentional external type exposure on all crates in workspace.
check-external-types-all toolchain="+nightly":
    #!/usr/bin/env bash
    set -euo pipefail
    exit=0
    for f in $(find . -mindepth 2 -maxdepth 2 -name Cargo.toml | grep -vE "\-codegen/|\-derive/|\-macros/"); do
        if ! just check-external-types-manifest "$f" {{toolchain}}; then exit=1; fi
        echo
        echo
    done
    exit $exit

# Check for unintentional external type exposure on all crates in workspace.
check-external-types-all-table toolchain="+nightly":
    #!/usr/bin/env bash
    set -euo pipefail
    for f in $(find . -mindepth 2 -maxdepth 2 -name Cargo.toml | grep -vE "\-codegen/|\-derive/|\-macros/"); do
        echo
        echo "Checking for $f"
        just check-external-types-manifest "$f" {{toolchain}} --output-format=markdown-table
    done

# Check for unintentional external type exposure on a crate.
check-external-types-manifest manifest_path toolchain="+nightly" *extra_args="":
    cargo {{toolchain}} check-external-types --manifest-path "{{manifest_path}}" {{extra_args}}
