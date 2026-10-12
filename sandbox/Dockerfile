# Compile-check sandbox: cross-compiles the Windows target from Linux so build/clippy runs
# isolated from the host. It cannot run the binaries (no WASAPI / NPU / WinML here);
# runtime spikes must run natively on each Windows machine.
FROM rust:1-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends clang lld llvm \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-pc-windows-msvc && rustup component add clippy rustfmt \
    && cargo install cargo-xwin --locked
WORKDIR /work
# Usage: see sandbox/check.ps1
