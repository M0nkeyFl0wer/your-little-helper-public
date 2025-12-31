# OAuth Solution for Little Helper

## Problem Solved

You wanted to use your Claude Max subscription (like Claude Code does) instead of API keys.

**Solution:** Reuse Claude Code's existing OAuth credentials!

## How It Works

1. **Claude Code maintains OAuth tokens** at `~/.claude/.credentials.json`
2. **Little Helper reads those tokens** and uses them for API calls
3. **Tokens are refreshed automatically** when they expire
4. **No separate authentication needed** - if you're signed into Claude Code, Little Helper works!

## Your Current Credentials

Found at `/home/flower/.claude/.credentials.json`:
- Subscription Type: **Claude Max** ✓
- Rate Limit: `default_claude_max_5x` (5x higher than standard)
- Scopes: `user:inference`, `user:profile`, `user:sessions:claude_code`
- Token expiration: Automatically refreshed by Claude Code

## Implementation

### Priority Order:
1. **First:** Check `~/.claude/.credentials.json` for OAuth tokens
2. **Second:** Check `ANTHROPIC_API_KEY` environment variable
3. **Third:** Fall back to Ollama/Gemini

### Token Structure:
```json
{
  "claudeAiOauth": {
    "accessToken": "sk-ant-oat01-...",
    "refreshToken": "sk-ant-ort01-...",
    "expiresAt": 1765617970362,
    "scopes": ["user:inference", "user:profile", "user:sessions:claude_code"],
    "subscriptionType": "max",
    "rateLimitTier": "default_claude_max_5x"
  }
}
```

### API Authentication:
- Use access token as `x-api-key` header (OAuth tokens use same format as API keys)
- Check expiration before each request
- If expired, use refresh token (or rely on Claude Code to refresh)

## Benefits

- No separate API key needed
- Uses your existing Claude Max subscription
- 5x higher rate limits than standard
- Automatic token refresh via Claude Code
- Single sign-on experience

## Fallback Strategy

If Claude Code credentials not found:
1. Warn user: "Claude Code not detected. Using API key fallback."
2. Check `ANTHROPIC_API_KEY` environment variable
3. If that fails, try Ollama
4. Finally try Gemini

This ensures Little Helper works even without Claude Code installed.

---

**Next Steps:**
1. Modify `claude.rs` to check `~/.claude/.credentials.json` first
2. Add token expiration checking
3. Test with your Claude Max subscription
4. Document setup process for team
