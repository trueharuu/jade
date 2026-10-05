#!/usr/bin/env bash
# Checks `jade_cli solve` against a recorded fixture.
#
# Fixture format (see tests/jigsaw):
#   <fumen> <pattern> <recorded_true>/<total>
#   <queue>,true|false      one line per queue the pattern expands to
#
# Usage:
#   tests/check-solve.sh <fixture> [extra solve args...]
#
# Extra args go to both `solve` and `percent` (for example --2l, --hold avoid,
# -s TS). The fixture header does not record them, so pass what the recorded
# result was produced with.
#
# Environment:
#   JADE_BIN  path to the jade_cli binary. Relative paths resolve from the
#             repo root. Default: target/{release,debug}/jade_cli.
#   JOBS      number of parallel workers. Default: nproc.
#
# Exit status: 0 = every queue matches, 1 = mismatches, 2 = bad usage.

set -eo pipefail

if [ "$#" -lt 1 ]; then
    echo "usage: $0 <fixture> [extra solve args...]" >&2
    exit 2
fi

fixture=$1
shift
solve_args=("$@")

if [ ! -r "$fixture" ]; then
    echo "error: cannot read $fixture" >&2
    exit 2
fi

# Make the fixture path absolute; the script changes directory below.
case $fixture in
    /*) ;;
    *) fixture=$PWD/$fixture ;;
esac

# Run from the repo root so `cargo` and `target/` resolve.
cd "$(dirname "$0")/.."

bin=${JADE_BIN:-}
if [ -z "$bin" ]; then
    if [ -x target/release/jade_cli ]; then
        bin=target/release/jade_cli
    elif [ -x target/debug/jade_cli ]; then
        bin=target/debug/jade_cli
    else
        cargo build --release -p jade_cli
        bin=target/release/jade_cli
    fi
fi

# Header: the first word is the field, the last word is the recorded count,
# and everything between is the pattern (patterns may contain spaces).
header=$(head -n 1 "$fixture" | tr -d '\r')
field=${header%% *}
recorded=${header##* }
pattern=${header#"$field" }
pattern=${pattern%" $recorded"}

if [ -z "$field" ] || [ -z "$pattern" ]; then
    echo "error: bad header: $header" >&2
    exit 2
fi
# `case` does the glob match; `test`'s `!=` only compares strings.
case $recorded in
    */*) ;;
    *)
        echo "error: bad recorded count in header: $header" >&2
        exit 2
        ;;
esac

jobs=${JOBS:-$(nproc 2>/dev/null || echo 4)}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
results=$tmp/results

# Runs one queue and prints "<queue> <recorded> <jade>".
# `solve` reports reachability with its exit status: 0 true, 1 false, and
# anything else is a crash (101 = panic), kept as "error<code>" so crashes
# are not mistaken for a plain false.
check_one() {
    local line=$1 queue want got rc
    queue=${line%,*}
    want=${line##*,}
    if "$bin" solve --field "$field" --pattern "$queue" \
        ${solve_args[@]+"${solve_args[@]}"} >/dev/null 2>&1; then
        got=true
    else
        rc=$?
        case $rc in
            1) got=false ;;
            *) got="error$rc" ;;
        esac
    fi
    printf '%s %s %s\n' "$queue" "$want" "$got"
}

echo "fixture:  $fixture"
echo "field:    $field"
echo "pattern:  $pattern"
echo "recorded: $recorded"
echo "args:     ${solve_args[*]:-(none)}"

# One `percent` run gives the aggregate answer before the per-queue work.
percent=$("$bin" percent --field "$field" --pattern "$pattern" \
    ${solve_args[@]+"${solve_args[@]}"})
echo "percent:  $percent (jade)"

# Check every queue line, a few workers at a time. The second condition
# keeps the last line when the fixture has no trailing newline.
running=0
while IFS= read -r line || [ -n "$line" ]; do
    line=${line%$'\r'}
    [ -n "$line" ] || continue
    check_one "$line" >>"$results" &
    running=$((running + 1))
    if [ "$running" -ge "$jobs" ]; then
        wait -n 2>/dev/null || wait || true
        running=$((running - 1))
    fi
done < <(tail -n +2 "$fixture")
wait || true

checked=$(wc -l <"$results")
recorded_total=${recorded##*/}
if [ "$checked" -ne "$recorded_total" ]; then
    echo "warning: fixture lists $recorded_total queues, checked $checked" >&2
fi

jade_true=$(awk '$3 == "true" { n++ } END { print n + 0 }' "$results")
errors=$(awk '$3 ~ /^error/ { n++ } END { print n + 0 }' "$results")

echo "queues:   $checked checked, $jade_true reachable (recorded ${recorded%/*})"
if [ "$errors" -ne 0 ]; then
    echo "warning: $errors solve run(s) exited with an error" >&2
fi
if [ "$jade_true" -ne "${percent%/*}" ]; then
    echo "warning: percent says ${percent%/*}, per-queue count says $jade_true" >&2
fi

mismatch=$(awk '$2 != $3' "$results")
if [ -z "$mismatch" ]; then
    echo "result:   OK"
    exit 0
fi

count=$(printf '%s\n' "$mismatch" | wc -l)
echo "result:   $count mismatch(es)"
printf '%s\n' "$mismatch" | sort | awk '{ printf "mismatch: %s recorded=%s jade=%s\n", $1, $2, $3 }'
exit 1
