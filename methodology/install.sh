#!/usr/bin/env sh
# Usage: install.sh link [project-root]   — symlink the conventional locations to methodology/ (relative links)
#        install.sh copy [project-root]   — copy files instead (no dependency on methodology/)
set -eu
MODE=${1:-}; ROOT=${2:-$(cd "$(dirname "$0")/.." && pwd)}
SRC=$(cd "$(dirname "$0")" && pwd)
[ "$MODE" = link ] || [ "$MODE" = copy ] || { echo "usage: $0 link|copy [project-root]" >&2; exit 2; }
cd "$ROOT"
mkdir -p .agents .claude .kiro/settings
place() { # $1 = target path in project, $2 = relative link, $3 = source path
  rm -rf "$1"
  if [ "$MODE" = link ]; then ln -s "$2" "$1"; else cp -R "$3" "$1"; fi
}
if [ "$MODE" = link ] && [ ! -d "$ROOT/methodology" ]; then echo "link mode needs $ROOT/methodology (submodule/subtree)" >&2; exit 2; fi
if [ "$MODE" = link ]; then
  # Plain pointer file, not a symlink: a tool that overwrites AGENTS.md in place
  # (no unlink, just write) would otherwise write through the symlink and
  # corrupt methodology/AGENTS.md, the actual source. A pointer keeps the
  # blast radius to these few lines.
  rm -f AGENTS.md
  printf '%s\n' \
    '# AGENTS.md — pointer, not the source' \
    '' \
    'Full methodology: `methodology/AGENTS.md` — read it before doing anything here.' \
    'Do not add content below this line: writes here do not reach the source and are lost on reinstall.' \
    > AGENTS.md
else
  place AGENTS.md methodology/AGENTS.md "$SRC/AGENTS.md"
fi
place .agents/skills            ../methodology/skills          "$SRC/skills"
place .claude/skills            ../methodology/skills          "$SRC/skills"
place .kiro/settings/templates  ../../methodology/templates    "$SRC/templates"
echo "installed ($MODE) into $ROOT"
