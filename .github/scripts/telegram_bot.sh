#!/bin/env bash
#
# Upload a build artifact to Telegram.
#
# Required env: BOT_TOKEN, CHAT_ID
# Optional env: TITLE, VERSION, COMMIT_MESSAGE, COMMIT_URL, RUN_URL
#
# The caption is sent as MarkdownV2, which rejects any message containing an
# unescaped special character (_ * [ ] ( ) ~ ` > # + - = | { } . !) — commit
# messages hit this constantly, so everything user-supplied is escaped first.

set -uo pipefail

file="${1:-}"

if [ ! -f "$file" ]; then
	echo "error: File not found: $file" >&2
	exit 1
fi

if [ -z "${BOT_TOKEN:-}" ] || [ -z "${CHAT_ID:-}" ]; then
	echo "error: BOT_TOKEN/CHAT_ID are not set" >&2
	exit 1
fi

# Escape MarkdownV2 special characters. Single pass, so a backslash introduced
# by this pass is never escaped again.
mdv2() {
	python3 - "$1" <<'PY'
import re, sys
print(re.sub(r'([_*\[\]()~`>#+\-=|{}.!\\])', r'\\\1', sys.argv[1]), end='')
PY
}

TITLE="${TITLE:-AZenith}"
VERSION="${VERSION:-0}"
COMMIT_MESSAGE="${COMMIT_MESSAGE:-$(git log -1 --pretty=%s 2>/dev/null || echo 'no commit message')}"
COMMIT_URL="${COMMIT_URL:-}"
RUN_URL="${RUN_URL:-}"

# MarkdownV2 captions are capped at 1024 characters.
if [ "${#COMMIT_MESSAGE}" -gt 600 ]; then
	COMMIT_MESSAGE="${COMMIT_MESSAGE:0:600}…"
fi

links=""
[ -n "$COMMIT_URL" ] && links+="[Commit]($COMMIT_URL)"$'\n'
[ -n "$RUN_URL" ] && links+="[Workflow run]($RUN_URL)"$'\n'

msg="*$(mdv2 "$TITLE")*
\\#ci\\_$VERSION
\`\`\`
$(mdv2 "$COMMIT_MESSAGE")
\`\`\`
$links"

if ! curl -sS --fail-with-body "https://api.telegram.org/bot$BOT_TOKEN/sendDocument" \
	-F document=@"$file" \
	-F chat_id="$CHAT_ID" \
	-F "disable_web_page_preview=true" \
	-F "parse_mode=markdownv2" \
	-F caption="$msg"; then
	echo
	echo "error: Telegram upload failed" >&2
	exit 1
fi
echo
