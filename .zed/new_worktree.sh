#!/bin/sh
set -eu

find "$ZED_MAIN_GIT_WORKTREE" -maxdepth 1 -type f -name '*.local.*' -exec sh -c '
  for file do
    dest="$ZED_WORKTREE_ROOT/${file##*/}"
    [ -e "$dest" ] || cp "$file" "$dest"
  done
' sh {} +

pnpm i
