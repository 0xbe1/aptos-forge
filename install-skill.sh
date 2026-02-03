#!/bin/sh
set -e
mkdir -p ~/.claude/skills/aptos
curl -sSL https://raw.githubusercontent.com/0xbe1/aptly/main/.claude/skills/aptos/SKILL.md \
  -o ~/.claude/skills/aptos/SKILL.md
echo "✓ Installed aptos skill to ~/.claude/skills/aptos/"
