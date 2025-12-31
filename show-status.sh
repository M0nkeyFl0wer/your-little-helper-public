#!/bin/bash

# Little Helper - Project Status Summary
# Shows current OAuth setup and project completion status

echo "╔════════════════════════════════════════════════════════════╗"
echo "║         Little Helper - OAuth Integration Complete!       ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Check Claude Code installation
if command -v claude &> /dev/null; then
    echo "✅ Claude Code is installed"
else
    echo "❌ Claude Code not found - install with: npm install -g @anthropic-ai/claude-code"
fi

# Check OAuth credentials
if [ -f ~/.claude/.credentials.json ]; then
    echo "✅ OAuth credentials found"

    # Extract details
    SUB_TYPE=$(jq -r '.claudeAiOauth.subscriptionType' ~/.claude/.credentials.json 2>/dev/null)
    RATE_LIMIT=$(jq -r '.claudeAiOauth.rateLimitTier' ~/.claude/.credentials.json 2>/dev/null)
    EXPIRES_AT=$(jq -r '.claudeAiOauth.expiresAt' ~/.claude/.credentials.json 2>/dev/null)

    if [ "$SUB_TYPE" != "null" ]; then
        echo "   Subscription: $SUB_TYPE"
        echo "   Rate Limit: $RATE_LIMIT"

        # Check expiration
        NOW=$(date +%s)000  # milliseconds
        if [ $EXPIRES_AT -gt $NOW ]; then
            echo "   Status: ✅ Token valid"
        else
            echo "   Status: ⚠️  Token expired - run 'claude' to refresh"
        fi
    fi
else
    echo "❌ No OAuth credentials - run 'claude' to sign in"
fi
echo ""

# Check build
if [ -f ./target/release/app ]; then
    echo "✅ Little Helper built (release mode)"
    SIZE=$(du -h ./target/release/app | cut -f1)
    echo "   Binary size: $SIZE"
else
    echo "⚠️  No release build found - run 'cargo build --release'"
fi
echo ""

# Project status
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                    Project Status                          ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""
echo "Track 1 (Simple App) - COMPLETE:"
echo "  ✅ Plain text editor (scratch pad)"
echo "  ✅ File viewer (images, HTML, text, PDFs)"
echo "  ✅ Claude Max OAuth integration"
echo "  ✅ Collapsible UI panels"
echo "  ✅ Image display with auto-scaling"
echo "  ✅ Token expiration checking"
echo ""

echo "Track 2 (VS Codium Fork) - PLANNED:"
echo "  📝 See /home/flower/Downloads/little-helper-vscodium/"
echo "  📝 Full implementation plan (1,179 lines)"
echo "  📝 Estimated: 4-8 weeks"
echo ""

# Quick Start
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                    Quick Start                             ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""
echo "Run Little Helper:"
echo "  ./target/release/app"
echo ""
echo "Expected output:"
echo "  ✓ Using Claude Code authentication: OAuth (max)"
echo ""

# Documentation
echo "Documentation Files:"
echo "  📄 README.md - Project overview"
echo "  📄 OAUTH_SETUP_COMPLETE.md - OAuth guide (PRIMARY)"
echo "  📄 OAUTH_SOLUTION.md - How OAuth reuse works"
echo "  📄 SESSION_SUMMARY.md - Complete feature list"
echo "  📄 MACALLISTER_SPEC.md - Original specification"
echo ""

# Test command
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                    Ready to Test!                          ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""
echo "Test OAuth authentication:"
echo "  ./test-oauth.sh"
echo ""
echo "Run Little Helper:"
echo "  ./target/release/app"
echo ""
