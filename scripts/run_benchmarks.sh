#!/usr/bin/env bash
# scripts/run_benchmarks.sh
#
# Builds the CortenMM_rw kernel image with the three benchmark
# programs in benchmarks/ packaged into its initramfs, boots it under
# QEMU/KVM, and runs each benchmark at the range sizes reported in the
# paper's Experimental Evaluation ("RQ4: Runtime Performance Impact").
#
# PREREQUISITE: a CortenMM_rw kernel tree (unmodified) under
# $KERNEL_ROOT, per docs/environment-setup-and-findings.md.
#
# This script documents the exact commands used; see that document
# for the pthread-linkage workaround these benchmark programs already
# incorporate, and for the cargo-osdk reinstall step that may be
# needed after copying these files into a fresh kernel tree.

set -euo pipefail

KERNEL_ROOT="${KERNEL_ROOT:-/root/asterinas/cortenmm-rw}"
SCALE_DIR="$KERNEL_ROOT/test/src/apps/scale"

mkdir -p "$SCALE_DIR"
cp "$(dirname "$0")"/../benchmarks/mmap_batch_scale.c "$SCALE_DIR/"
cp "$(dirname "$0")"/../benchmarks/mprotect_scale.c "$SCALE_DIR/"
cp "$(dirname "$0")"/../benchmarks/alloc_arena_scale.c "$SCALE_DIR/"

cd "$KERNEL_ROOT"
rm -rf target/osdk test/build
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
