# Little Helper - Setup Guide

## For Team Deployment (MacAllister Polling)

### Current Configuration
The app currently uses:
1. **Ollama** (local AI model) - requires installation
2. **Gemini API** (fallback) - requires API key

### Recommended: Claude API (Plug & Play)

**Why Claude API?**
- ✅ No local installation needed
- ✅ Better quality responses
- ✅ One team API key for everyone
- ✅ Works immediately on any Mac
- ✅ Predictable billing

**Cost Estimate:**
- **Haiku** (fast, cheap): $0.25 input / $1.25 output per million tokens
  - Typical usage: ~$10-20/month per user
- **Sonnet 4.5** (smarter): $3 input / $15 output per million tokens
  - Typical usage: ~$30-50/month per user

For 5 team members: **$50-250/month total** (depending on model choice)

---

## Setup Options

### Option 1: Pre-Configured Team Build (Recommended)

**For team admin:**
1. Get Claude API key from Anthropic (https://console.anthropic.com/)
2. Add key to `settings.json` or build-time config
3. Bundle app with key already configured
4. Distribute .app file to team

**For team members:**
1. Download .app file
2. Move to Applications folder
3. Double-click to run
4. **Works immediately - no setup!**

### Option 2: Individual API Keys

Each team member:
1. Gets their own Claude API key
2. First run: App prompts for API key
3. Saves key locally
4. Ready to use

---

## Current Code Changes Needed

### 1. Add Claude API Support

File: `crates/shared/src/agent_api.rs`

```rust
pub struct ApiConfig {
    pub provider: Provider,
    pub api_key: Option<String>,
}

pub enum Provider {
    Ollama,
    Gemini,
    Claude,  // New
}

// Add Claude API client implementation
impl AgentApi {
    pub async fn chat_claude(&self, messages: Vec<ChatMessage>) -> Result<String> {
        // Use anthropic-sdk or direct HTTP calls
        let client = anthropic::Client::new(self.config.api_key.as_ref()?);
        let response = client.messages().create(
            CreateMessageRequest {
                model: "claude-sonnet-4-5-20250929",
                messages: messages.into_iter().map(|m| {
                    anthropic::Message {
                        role: m.role,
                        content: m.content,
                    }
                }).collect(),
                max_tokens: 4096,
            }
        ).await?;
        Ok(response.content[0].text.clone())
    }
}
```

### 2. Add API Key Configuration UI

File: `crates/app/src/main.rs`

```rust
// Add settings panel
if !api_key_configured {
    show_api_key_setup_dialog(ui);
}

fn show_api_key_setup_dialog(ui: &mut egui::Ui) {
    ui.heading("Welcome to Little Helper!");
    ui.label("Please enter your Claude API key to get started:");
    ui.text_edit_singleline(&mut api_key_input);
    if ui.button("Save & Continue").clicked() {
        save_api_key(api_key_input);
    }
}
```

### 3. Add Anthropic SDK Dependency

File: `Cargo.toml`

```toml
[dependencies]
anthropic-sdk = "0.1"  # Or use reqwest for direct HTTP calls
```

---

## Distribution Process

### Mac .app Bundle Creation

```bash
# 1. Build release binary
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# 2. Create universal binary
lipo -create \
  target/x86_64-apple-darwin/release/app \
  target/aarch64-apple-darwin/release/app \
  -output LittleHelper

# 3. Create .app structure
mkdir -p LittleHelper.app/Contents/MacOS
mkdir -p LittleHelper.app/Contents/Resources

# 4. Copy binary
cp LittleHelper LittleHelper.app/Contents/MacOS/

# 5. Create Info.plist
cat > LittleHelper.app/Contents/Info.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>LittleHelper</string>
    <key>CFBundleIdentifier</key>
    <string>com.mcallister.littlehelper</string>
    <key>CFBundleName</key>
    <string>Little Helper</string>
    <key>CFBundleVersion</key>
    <string>1.0.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

# 6. Optional: Code sign
codesign --force --deep --sign "Developer ID Application: Your Name" LittleHelper.app

# 7. Create DMG for distribution
hdiutil create -volname "Little Helper" -srcfolder LittleHelper.app -ov -format UDZO LittleHelper.dmg
```

---

## Security Note

**API Key Storage:**
- Store in macOS Keychain (most secure)
- Or encrypted local file
- Never commit keys to git
- Use environment variable override for testing

**Code:**
```rust
use keyring::Entry;

fn save_api_key(key: &str) -> Result<()> {
    let entry = Entry::new("LittleHelper", "claude_api_key")?;
    entry.set_password(key)?;
    Ok(())
}

fn load_api_key() -> Result<String> {
    let entry = Entry::new("LittleHelper", "claude_api_key")?;
    entry.get_password()
}
```

---

## Next Steps

1. **Decide on approach:**
   - Pre-configured team build? (easiest for users)
   - Individual API keys? (more flexible)

2. **Get API key:**
   - Sign up at https://console.anthropic.com/
   - Choose billing plan
   - Generate API key

3. **Test integration:**
   - Add Claude API support to code
   - Test with your key
   - Verify cost/usage

4. **Build & distribute:**
   - Create .app bundle
   - Share with team
   - Collect feedback

---

**Questions to answer:**
1. Should we use pre-configured team key or individual keys?
2. Which model? (Haiku for speed/cost vs Sonnet for quality)
3. What's the monthly budget for AI costs?
4. Do you have a Mac to build the .app bundle?

