#!/usr/bin/env bash
# scripts/run_benchmarks.sh
#
# Builds the CortenMM_rw kernel image with the three benchmark
# programs in benchmarks/ packaged into its initramfs, boots it under
# QEMU/KVM, and runs each benchmark at the range sizes reported in the
# paper's Experimental Evaluation ("RQ4: Runtime Performance Impact").
#
# PREREQUISITE: a CortenMM_rw kernel tree (unmodified) under
# $KERNEL_ROOT.
#
# This script applies a required build-tooling patch before building
# (see docs/environment-setup-and-findings.md, issue 4, for the full
# root-cause diagnosis): cargo-osdk's generated manifest for the
# kernel's run-base wrapper crate specifies osdk-frame-allocator,
# osdk-heap-allocator, and ostd as version-only dependencies; if the
# registry mirror's local cache for the first two happens to be warm,
# Cargo resolves a SECOND, registry-published copy of ostd alongside
# the local, path-linked one the kernel depends on directly, causing
# a #[global_allocator]/#[alloc_error_handler] "conflicts with ostd"
# build failure. This is intermittent (depends on unrelated cargo
# activity's effect on the registry cache), which is why it may not
# reproduce on every attempt without this patch.

set -euo pipefail

KERNEL_ROOT="${KERNEL_ROOT:-/root/asterinas/cortenmm-rw}"
SCALE_DIR="$KERNEL_ROOT/test/src/apps/scale"
TEMPLATE="$KERNEL_ROOT/osdk/src/base_crate/Cargo.toml.template"

mkdir -p "$SCALE_DIR"
cp "$(dirname "$0")"/../benchmarks/mmap_batch_scale.c "$SCALE_DIR/"
cp "$(dirname "$0")"/../benchmarks/mprotect_scale.c "$SCALE_DIR/"
cp "$(dirname "$0")"/../benchmarks/alloc_arena_scale.c "$SCALE_DIR/"

# Apply the osdk-frame-allocator/osdk-heap-allocator/ostd local-path
# patch idempotently (skip if already applied by a previous run).
if ! grep -q "osdk-frame-allocator = " "$TEMPLATE"; then
  cat >> "$TEMPLATE" << TEMPLATE_EOF

# Force these three to resolve to the local workspace copies rather
# than the registry mirror (see docs/environment-setup-and-findings.md,
# issue 4, for why this is required).
osdk-frame-allocator = { path = "$KERNEL_ROOT/osdk/deps/frame-allocator" }
osdk-heap-allocator = { path = "$KERNEL_ROOT/osdk/deps/heap-allocator" }
ostd = { path = "$KERNEL_ROOT/ostd" }
TEMPLATE_EOF
  echo "Applied osdk manifest patch to $TEMPLATE"
fi

# The template above is compiled into the cargo-osdk binary, so it
# must be reinstalled from this tree's own osdk/ for the patch to
# take effect.
(cd "$KERNEL_ROOT/osdk" && cargo install --path . --force --locked)

cd "$KERNEL_ROOT"
rm -rf target test/build
timeout 600 make build SMP=1 MEM=8G RELEASE_LTO=1

echo "Build complete. Boot the image (e.g. 'make run SMP=1 MEM=8G"
echo "RELEASE_LTO=1' in a separate session/tmux pane) and, at the"
echo "guest shell prompt, run:"
echo ""
echo "  /test/scale/mmap_batch_scale <N>       for N in 1 4 16 32 64"
echo "  /test/scale/mprotect_scale <N>         for N in 4 16 32 64"
echo "  /test/scale/alloc_arena_scale <N> <P>  for (N,P) in"
echo "                                         (4,4) (16,4) (64,4) (64,16)"
echo ""
echo "Each program prints BATCHED_CYCLES / PER_PAGE_CYCLES (or"
echo "ARENA_CYCLES / DIRECT_CYCLES) and the overhead ratio directly;"
echo "compare against benchmarks/raw_data_*.csv."
