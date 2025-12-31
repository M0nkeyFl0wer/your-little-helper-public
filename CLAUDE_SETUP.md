# Claude API Setup for Little Helper

## Quick Start

You can now use your Claude Max account directly with Little Helper!

### Step 1: Get Your API Key

1. Go to https://console.anthropic.com/
2. Log in with your Claude account
3. Navigate to "API Keys"
4. Create a new key or copy your existing key

### Step 2: Set Environment Variable

**On Linux/Mac:**
```bash
export ANTHROPIC_API_KEY="your-api-key-here"

# Or add to your shell profile (~/.bashrc or ~/.zshrc):
echo 'export ANTHROPIC_API_KEY="your-api-key-here"' >> ~/.bashrc
source ~/.bashrc
```

**On Mac (alternative):**
```bash
# Add to ~/.zshrc or ~/.bash_profile
echo 'export ANTHROPIC_API_KEY="your-api-key-here"' >> ~/.zshrc
source ~/.zshrc
```

### Step 3: Run Little Helper

```bash
/home/flower/Downloads/claudia-lite/target/release/app
```

The app will automatically use Claude if the API key is set!

---

## Provider Priority

Little Helper tries providers in this order:

1. **Claude** (if `ANTHROPIC_API_KEY` is set) - Best quality
2. **Ollama** (local) - Free, private, but requires setup
3. **Gemini** (if `GEMINI_API_KEY` is set) - Google's AI

If Claude API key is set, it will be used first for all chat requests.

---

## Supported Models

The default Claude model is **claude-sonnet-4-5-20250929** (Sonnet 4.5)

To use a different model, you'll need to modify the code or add a config option.

Available models:
- `claude-sonnet-4-5-20250929` - Latest Sonnet (recommended)
- `claude-opus-4-5-20251101` - Opus 4.5 (highest quality, slower)
- `claude-haiku-4-20250514` - Haiku 4 (fastest, cheapest)

---

## Costs (Approximate)

**With Claude Max subscription:**
- Typically includes generous API credits
- Additional usage billed to your account
- Check your console for current rates

**Without subscription:**
- Pay-as-you-go pricing
- Sonnet: ~$3 input / $15 output per million tokens
- Haiku: ~$0.25 input / $1.25 output per million tokens

For casual use: **~$10-30/month**
For heavy use: **~$50-100/month**

---

## Configuration File (Future)

In a future update, you'll be able to configure Claude in `settings.json`:

```json
{
  "ai_provider": "claude",
  "claude_model": "claude-sonnet-4-5-20250929",
  "fallback_to_ollama": true
}
```

---

## Troubleshooting

### "ANTHROPIC_API_KEY not set"
- Make sure you exported the environment variable
- Restart your terminal after adding to profile
- Verify: `echo $ANTHROPIC_API_KEY`

### "Claude API error 401"
- API key is invalid or expired
- Generate a new key at https://console.anthropic.com/

### "All AI providers failed"
- No providers are configured
- Set either `ANTHROPIC_API_KEY`, install Ollama, or set `GEMINI_API_KEY`

---

## Testing Your Setup

```bash
# Check if API key is set
echo $ANTHROPIC_API_KEY

# Should show your key (keep it secret!)
# If empty, the key is not set

# Run the app
/home/flower/Downloads/claudia-lite/target/release/app

# Open chat and send a message
# You should see responses from Claude!
```

---

## Security Notes

- **Never commit API keys to git**
- Keep your API key private
- Rotate keys regularly
- Use environment variables, not hardcoded keys
- Consider using macOS Keychain for production

---

**Last updated:** 2025-12-12
