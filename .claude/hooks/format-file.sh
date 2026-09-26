#!/bin/bash
# Auto-format files after Edit/Write tool use

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

[ -z "$FILE_PATH" ] && exit 0
[ ! -f "$FILE_PATH" ] && exit 0

if [[ "$FILE_PATH" == *.rs ]]; then
  # rust-toolchain.toml と rustfmt.toml を拾うためにプロジェクトルートで実行する
  cd "$CLAUDE_PROJECT_DIR" && rustfmt "$FILE_PATH" 2>/dev/null
fi

exit 0
