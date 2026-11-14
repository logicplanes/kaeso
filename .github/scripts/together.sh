#!/usr/bin/env bash
# Runs every "name=command" argument at the same time, prints each one's output as a group, and fails when any of them failed.
set -uo pipefail

logs="$(mktemp -d)"
names=()
pids=()
for task in "$@"; do
  names+=("${task%%=*}")
  bash -euo pipefail -c "${task#*=}" > "$logs/${#pids[@]}.log" 2>&1 &
  pids+=("$!")
done

failed=()
for i in "${!pids[@]}"; do
  if wait "${pids[$i]}"; then
    echo "::group::${names[$i]}: passed"
    cat "$logs/$i.log"
    echo "::endgroup::"
  else
    echo "${names[$i]}: failed"
    cat "$logs/$i.log"
    failed+=("${names[$i]}")
  fi
done

for name in "${failed[@]}"; do
  echo "::error::$name failed."
done
[[ ${#failed[@]} -eq 0 ]]
