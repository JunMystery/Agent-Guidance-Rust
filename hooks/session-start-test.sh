#!/bin/bash
# session-start-test.sh - Tests for the SessionStart hook JSON payload

set -euo pipefail

tmp_payload="$(mktemp)"
trap 'rm -f "$tmp_payload"' EXIT

has_jq=0
if command -v jq >/dev/null 2>&1; then
  has_jq=1
fi

payload="$(bash hooks/session-start.sh)"
printf '%s' "$payload" > "$tmp_payload"

HAS_JQ="$has_jq" PAYLOAD_PATH="$tmp_payload" node <<'NODE'
const fs = require('fs');

const payload = JSON.parse(fs.readFileSync(process.env.PAYLOAD_PATH, 'utf8'));
const hasJq = process.env.HAS_JQ === '1';

if (payload.priority !== 'INFO' && payload.priority !== 'IMPORTANT') {
  throw new Error(`expected INFO or IMPORTANT priority, got ${payload.priority}`);
}

if (!payload.message.includes('agent-guidance') && !payload.message.includes('agent-skills')) {
  throw new Error('message is missing expected session content');
}

console.log('session-start JSON payload OK');
NODE
