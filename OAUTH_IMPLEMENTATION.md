# OAuth Authentication for Little Helper

## Goal
Allow users to sign in with their Claude.ai account (like Claude Code does) instead of using API keys.

## How Claude Code Does It

Claude Code uses OAuth 2.0 with Anthropic's authentication service:

1. User clicks "Sign in with Claude"
2. Opens browser to Anthropic's OAuth page
3. User authorizes the application
4. App receives OAuth token
5. Token is used for API calls (linked to Claude Max subscription)

## Implementation Steps

### 1. Register OAuth Application with Anthropic

Need to contact Anthropic to register the application and get:
- Client ID
- Client Secret
- Redirect URI

### 2. Implement OAuth Flow

```rust
// OAuth flow:
// 1. Generate authorization URL
// 2. Open browser to Anthropic's auth page
// 3. User authorizes
// 4. Receive callback with authorization code
// 5. Exchange code for access token
// 6. Store token securely
// 7. Use token for API calls
```

### 3. Token Storage

Store OAuth tokens securely:
- **Mac**: Keychain
- **Linux**: Secret Service / gnome-keyring
- **Windows**: Credential Manager

### 4. Token Refresh

OAuth tokens expire and need to be refreshed:
- Store refresh token
- Automatically refresh when access token expires
- Handle re-authentication if refresh fails

---

## Alternative: Use Claude Code's Authentication

Instead of implementing OAuth ourselves, we could:

1. **Detect if Claude Code is installed**
2. **Reuse Claude Code's authentication**
3. **Use the same tokens**

This would work if:
- User has Claude Code installed
- We can access Claude Code's token storage
- We use the same OAuth client ID (may require Anthropic approval)

---

## Simpler Alternative: Desktop OAuth Flow

Use a local HTTP server for OAuth callback:

```rust
use oauth2::{AuthorizationCode, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
use oauth2::basic::BasicClient;

// 1. Start local HTTP server on http://localhost:8080
// 2. Generate OAuth URL
let auth_url = client
    .authorize_url(CsrfToken::new_random)
    .url();

// 3. Open browser
open::that(auth_url.as_str())?;

// 4. Receive callback at localhost:8080
// 5. Extract authorization code
// 6. Exchange for token
let token = client
    .exchange_code(AuthorizationCode::new(code))
    .request_async(async_http_client)
    .await?;
```

---

## What We Need from Anthropic

To implement this properly, we need:

1. **OAuth Client Credentials**
   - Apply for OAuth client ID/secret
   - Probably need to be approved by Anthropic
   - May need to explain use case (team tool for polling company)

2. **Documentation**
   - OAuth endpoints (authorization URL, token URL)
   - Scopes required
   - Token refresh process

3. **Permissions**
   - May need special permission to use Claude Max subscriptions via OAuth
   - Different from standard API access

---

## Quick Implementation Plan

### Phase 1: Research (1-2 hours)
- [ ] Check if Anthropic has public OAuth documentation
- [ ] Look at Claude Code source (if available)
- [ ] Determine if we can reuse Claude Code auth
- [ ] Check if `anthropic-sdk` supports OAuth

### Phase 2: Register App (1-2 days)
- [ ] Contact Anthropic support
- [ ] Request OAuth client credentials
- [ ] Explain use case (internal team tool)
- [ ] Get approval

### Phase 3: Implement OAuth (1-2 days)
- [ ] Add `oauth2` crate dependency
- [ ] Implement authorization flow
- [ ] Add local HTTP server for callback
- [ ] Store tokens securely (keyring)
- [ ] Add token refresh logic

### Phase 4: UI Integration (1 day)
- [ ] Add "Sign in with Claude" button
- [ ] Show authentication status
- [ ] Handle token expiration gracefully
- [ ] Add "Sign out" option

---

## Dependencies Needed

```toml
[dependencies]
oauth2 = "4.4"
keyring = "2.0"  # Secure token storage
open = "5.0"     # Open browser
tiny_http = "0.12"  # Local server for OAuth callback
```

---

## Security Considerations

1. **Token Storage**
   - Never store tokens in plain text
   - Use OS keychain/credential manager
   - Encrypt tokens at rest

2. **Redirect URI**
   - Use `http://localhost:8080/callback` for desktop
   - Must match registered redirect URI exactly

3. **CSRF Protection**
   - Use state parameter to prevent CSRF attacks
   - Validate state in callback

---

## Current Blocker

**We don't have OAuth credentials from Anthropic.**

To move forward, we need to:
1. Contact Anthropic at api-support@anthropic.com
2. Request OAuth client ID/secret for desktop application
3. Explain this is for internal team use at MacAllister Polling

**Or:**

Check if we can leverage Claude Code's authentication (if they allow it).

---

## Temporary Solution

While waiting for OAuth approval, we could:
1. Keep the API key option as fallback
2. Add a "Sign in with Claude Code" option if installed
3. Document the OAuth plan for future implementation

---

**Next Step:** Should I contact Anthropic support to request OAuth credentials, or should we explore reusing Claude Code's auth first?
