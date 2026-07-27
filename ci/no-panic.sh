#!/usr/bin/env bash
# Fails if any panicking path survives in the compiled library.
#
# A single reachable panic pulls `core::panicking` and the formatting machinery into every binary
# that links this crate, and breaks users who forbid panics outright. Everything on `OnceCell<T>`
# is generic, so an ordinary build emits no code to inspect; `--cfg no_panic_check` compiles
# `src/no_panic_check.rs`, which instantiates the public API with a concrete type.
set -euo pipefail

target="${1:-thumbv7em-none-eabihf}"
cd "$(dirname "$0")/.."

RUSTFLAGS="--cfg no_panic_check" cargo build --release --lib --target "$target" --quiet
rlib="target/$target/release/libonce_cell_no_std.rlib"

# the rlib also contains `.rmeta` members, which nm cannot read; ignore its complaints
if symbols="$(nm "$rlib" 2>/dev/null | grep -iE 'panicking|panic_fmt|expect_failed|unwrap_failed')"; then
    echo "error: a panicking path survived in once_cell_no_std ($target):" >&2
    echo "$symbols" >&2
    exit 1
fi

echo "ok: no panicking paths in once_cell_no_std ($target)"
