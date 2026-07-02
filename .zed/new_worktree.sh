#!/bin/sh
set -eu

stash_ref=

if ! git -C "$ZED_MAIN_GIT_WORKTREE" diff --quiet \
  || ! git -C "$ZED_MAIN_GIT_WORKTREE" diff --cached --quiet \
  || [ -n "$(git -C "$ZED_MAIN_GIT_WORKTREE" ls-files --others --exclude-standard)" ]; then
  git -C "$ZED_MAIN_GIT_WORKTREE" stash push --include-untracked -m "zed new worktree handoff"
  stash_ref=$(git -C "$ZED_MAIN_GIT_WORKTREE" rev-parse --verify stash@{0})

  git -C "$ZED_MAIN_GIT_WORKTREE" stash apply --index "$stash_ref"
  git -C "$ZED_WORKTREE_ROOT" stash apply --index "$stash_ref"
  git -C "$ZED_MAIN_GIT_WORKTREE" stash drop "$stash_ref"
fi

pnpm i
