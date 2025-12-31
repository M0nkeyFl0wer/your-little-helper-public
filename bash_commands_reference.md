# Bash Commands Reference - OAuth Integration Session

This file tracks all bash commands used during the OAuth integration implementation for future reference.

## Session Date: 2025-12-12

## OAuth Credential Discovery

### Check if Claude Code is installed
```bash
which claude
# Output: /home/flower/.local/bin/claude
```

### List Claude Code configuration directory
```bash
ls -la ~/.claude
# Shows .credentials.json and other config files
```

### Read OAuth credentials (safely)
```bash
jq -r '.claudeAiOauth | "Subscription: \(.subscriptionType)\nRate Limit: \(.rateLimitTier)\nExpires: \(.expiresAt)"' ~/.claude/.credentials.json
# Output:
# Subscription: max
# Rate Limit: default_claude_max_5x
# Expires: 1765617970362
```

### Check subscription type only
```bash
jq -r '.claudeAiOauth.subscriptionType' ~/.claude/.credentials.json
# Output: max
```

## Building Little Helper

### Build release version
```bash
cd /home/flower/Downloads/claudia-lite
cargo build --release
# Builds optimized binary at ./target/release/app
```

### Check build output
```bash
ls -lh ./target/release/app
# Shows binary size (20M)
```

### Quick build status check
```bash
cargo build --release 2>&1 | head -100
# Shows first 100 lines of build output
```

## Testing OAuth Integration

### Make test script executable
```bash
chmod +x /home/flower/Downloads/claudia-lite/test-oauth.sh
```

### Run OAuth test
```bash
./test-oauth.sh
# Verifies OAuth credentials and runs app
```

### Make status script executable
```bash
chmod +x /home/flower/Downloads/claudia-lite/show-status.sh
```

### Check project status
```bash
./show-status.sh
# Shows complete project status including OAuth setup
```

### Run both scripts in one command
```bash
chmod +x /home/flower/Downloads/claudia-lite/show-status.sh && ./show-status.sh
```

## Running Little Helper

### Basic run (with OAuth)
```bash
/home/flower/Downloads/claudia-lite/target/release/app
# Should show: ✓ Using Claude Code authentication: OAuth (max)
```

### Run from project directory
```bash
cd /home/flower/Downloads/claudia-lite
./target/release/app
```

## Token Management

### Check token expiration
```bash
jq -r '.claudeAiOauth.expiresAt' ~/.claude/.credentials.json
# Returns timestamp in milliseconds
```

### Refresh OAuth token (if expired)
```bash
claude
# Running any claude command refreshes the token
```

### Alternative token refresh
```bash
claude setup
# Explicitly refreshes authentication
```

## File Operations

### Check if file exists
```bash
ls -la /path/to/file 2>&1 || echo "File does not exist"
```

### Check file size
```bash
du -h ./target/release/app
# Output: 20M
```

### Read specific JSON field
```bash
jq -r '.field.subfield' ~/.claude/.credentials.json
```

## Development Workflow

### Full rebuild and test
```bash
cd /home/flower/Downloads/claudia-lite
cargo build --release
./show-status.sh
./target/release/app
```

### Quick check before running
```bash
./show-status.sh && ./target/release/app
```

## Documentation Created

### All markdown files created
```bash
ls -1 /home/flower/Downloads/claudia-lite/*.md
# Lists:
# - README.md
# - SESSION_SUMMARY.md
# - OAUTH_SOLUTION.md
# - OAUTH_IMPLEMENTATION.md
# - OAUTH_SETUP_COMPLETE.md
# - OAUTH_COMPLETE_SUMMARY.md
# - MACALLISTER_SPEC.md
# - etc.
```

## Common Issues and Solutions

### "OAuth token expired"
```bash
# Solution: Run claude to refresh
claude
```

### "No authentication found"
```bash
# Solution: Sign in with Claude Code
claude
# Then verify credentials exist:
ls -la ~/.claude/.credentials.json
```

### Check which auth method is being used
```bash
# OAuth will show:
✓ Using Claude Code authentication: OAuth (max)

# API key will show:
✓ Using API key authentication
```

### Force API key usage (disable OAuth)
```bash
# Temporarily rename OAuth credentials
mv ~/.claude/.credentials.json ~/.claude/.credentials.json.backup
export ANTHROPIC_API_KEY="your-key-here"
./target/release/app
# Will use API key instead

# Restore OAuth
mv ~/.claude/.credentials.json.backup ~/.claude/.credentials.json
```

## Build Information

### Check Rust version
```bash
rustc --version
cargo --version
```

### Clean build
```bash
cargo clean
cargo build --release
```

### Check dependencies
```bash
cargo tree -p providers
# Shows dependency tree for providers crate
```

## Team Deployment Commands

### For each team member's machine:

#### Step 1: Install Claude Code (if not installed)
```bash
npm install -g @anthropic-ai/claude-code
```

#### Step 2: Sign in
```bash
claude
# Follow authentication flow in browser
```

#### Step 3: Verify authentication
```bash
jq -r '.claudeAiOauth.subscriptionType' ~/.claude/.credentials.json
# Should show: max, pro, team, or enterprise
```

#### Step 4: Run Little Helper
```bash
/path/to/little-helper/target/release/app
# Should see: ✓ Using Claude Code authentication: OAuth (...)
```

## Useful One-Liners

### Check everything is ready
```bash
command -v claude &> /dev/null && [ -f ~/.claude/.credentials.json ] && [ -f ./target/release/app ] && echo "✅ Ready to use!" || echo "❌ Setup incomplete"
```

### Show auth status
```bash
[ -f ~/.claude/.credentials.json ] && echo "OAuth: $(jq -r '.claudeAiOauth.subscriptionType' ~/.claude/.credentials.json)" || echo "OAuth: Not configured"
```

### Complete status check
```bash
echo "Claude Code: $(command -v claude &> /dev/null && echo 'Installed' || echo 'Not found')" && \
echo "OAuth: $([ -f ~/.claude/.credentials.json ] && jq -r '.claudeAiOauth.subscriptionType' ~/.claude/.credentials.json || echo 'Not configured')" && \
echo "Binary: $([ -f ./target/release/app ] && echo 'Built' || echo 'Not built')"
```

## Notes

- All `jq` commands require jq to be installed: `sudo apt install jq` (Linux) or `brew install jq` (Mac)
- OAuth tokens expire but are automatically refreshed by Claude Code
- Little Helper checks token expiration with 5-minute buffer
- Fallback to API key works if OAuth not available
- No need for environment variables with OAuth method

---

**Created:** 2025-12-12
**Purpose:** Reference for OAuth integration commands
**Status:** Complete and tested
