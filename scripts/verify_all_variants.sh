#!/usr/bin/env bash
# scripts/verify_all_variants.sh
#
# Re-verifies all six variants and prints each one's obligation/error
# count, reproducing the numbers in benchmarks/verification_obligations_summary.csv
# and the paper's Tables "Proof cost, CortenMM_rw vs. CortenMM_coarse",
# "...vs. CortenMM_nontxn", "The 2x2 design matrix", and
# "Proof cost, CortenMM_rcu vs. CortenMM_rcu-nontxn".
#
# PREREQUISITE: this script assumes the six crate directories listed
# below already exist under $VERIFICATION_ROOT (see docs/
# environment-setup-and-findings.md for how they were originally
# constructed: lock-protocol-rw and lock-protocol-rcu are the
# CortenMM-published baselines; the other four are built by applying
# the patches in patches/ to copies of the appropriate baseline, per
# the paper's Methods \S "Constructing the Six Variants").

set -uo pipefail
# NOTE: deliberately NOT using `set -e` here. Three of the six
# variants (lock-protocol-nontxn, lock-protocol-coarse-nontxn,
# lock-protocol-rcu-nontxn) have documented, expected residual errors
# (see docs/verus-tooling-issue.md) and `cargo xtask verify` exits
# non-zero for them; with `set -e` this script would silently abort
# after the first such variant and never reach the remaining ones.

VERIFICATION_ROOT="${VERIFICATION_ROOT:-/root/asterinas/verification}"

VARIANTS=(
  lock-protocol-rw
  lock-protocol-coarse
  lock-protocol-nontxn
  lock-protocol-coarse-nontxn
  lock-protocol-rcu
  lock-protocol-rcu-nontxn
)

cd "$VERIFICATION_ROOT"

for v in "${VARIANTS[@]}"; do
  echo "=== $v ==="
  output=$(cargo xtask verify --targets "$v" 2>&1)
  echo "$output" | grep "verification results::" || echo "$output" | tail -5
  echo ""
done
