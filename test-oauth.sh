#!/bin/bash

# Test OAuth Authentication for Little Helper
# This script verifies that Claude Code OAuth credentials are being used

echo "=== Testing OAuth Authentication ==="
echo ""

# Check if Claude Code credentials exist
if [ -f ~/.claude/.credentials.json ]; then
    echo "✓ Found Claude Code credentials at ~/.claude/.credentials.json"

    # Extract subscription type
    SUB_TYPE=$(jq -r '.claudeAiOauth.subscriptionType' ~/.claude/.credentials.json)
    EXPIRES_AT=$(jq -r '.claudeAiOauth.expiresAt' ~/.claude/.credentials.json)

    echo "  Subscription: $SUB_TYPE"
    echo "  Token expires: $(date -d @$((EXPIRES_AT / 1000)) 2>/dev/null || echo 'Check manually')"
    echo ""
else
    echo "✗ No Claude Code credentials found"
    echo "  Run 'claude' to sign in first"
    exit 1
fi

# Check if ANTHROPIC_API_KEY is set (should be ignored if OAuth available)
if [ -n "$ANTHROPIC_API_KEY" ]; then
    echo "⚠ ANTHROPIC_API_KEY is set, but OAuth will be used instead"
    echo ""
fi

echo "=== Running Little Helper ==="
echo "Expected: Should show 'Using Claude Code authentication: OAuth (max)'"
echo ""

# Run the app (will open GUI)
./target/release/app 2>&1 | grep -E "Using Claude|OAuth|authentication" || echo "Check GUI for authentication status"
