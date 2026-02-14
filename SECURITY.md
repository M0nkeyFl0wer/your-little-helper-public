# 🔒 Little Helper - Security Assessment

## Security Overview

Little Helper is designed with privacy-first principles: it is local-first by default, and network access is optional.

If you enable Internet research and/or configure a cloud model provider (e.g. OpenAI/Anthropic/Gemini), the app will make network requests to those services. If you use only local models and keep Internet research disabled, the app can run fully offline.

## Security Features ✅

### Data Privacy
- **Local-First**: Local models can be used without sending prompts to third parties
- **No Telemetry**: No usage data or analytics collected
- **Network Calls Are Optional**: Only when Internet research is enabled and/or a cloud provider is configured
- **Local File Access**: File searches stay on the local machine

### Application Security
- **Sandboxed File Access**: Only searches user-approved directories
- **Memory Safe**: Written in Rust (memory-safe language)
- **Minimal Dependencies**: Limited attack surface
- **No Elevated Privileges**: Runs as regular user

## Security Risks ⚠️

### Installation Risks (Temporary)
1. **`curl | bash` Pattern**: Downloads and executes installer script
   - **Mitigation**: Secure installer verifies sources and performs checks
   - **Alternative**: Manual installation instructions provided

2. **System-Wide Dependencies**: Installs Ollama and potentially Rust
   - **Risk**: Requires admin password, installs system binaries
   - **Mitigation**: Uses official installers from trusted sources

3. **Auto-Start Service**: Creates LaunchAgent for Ollama
   - **Risk**: Service runs automatically at startup
   - **Mitigation**: Service only runs Ollama (AI engine), no network access

### Runtime Risks (Low)
4. **Local AI Model**: Processes user queries and file content
    - **Risk**: AI could theoretically be prompted to reveal sensitive info
    - **Mitigation**: Keep Internet research disabled for fully offline operation

5. **Optional Terminal Command Execution**: If enabled, the assistant can run commands on your machine
   - **Risk**: Misuse could modify files, leak data via commands, or operate in unintended folders
   - **Mitigation**: Restrict allowed folders, require explicit approval for sensitive actions, and treat this feature as security-critical

6. **Full Disk Access**: App requests broad file system access
    - **Risk**: Could read sensitive files if compromised
    - **Mitigation**: macOS permissions system controls access

## Risk Mitigation

### For Installation
- Use the secure installer (`install-mac-safe.sh`) instead of basic one
- Review installer source code before running
- Manual installation option available for security-conscious users

### For Runtime
- macOS Gatekeeper prevents unsigned apps (we should code-sign)
- macOS permissions system controls file access
- Regular security updates via GitHub releases

## Recommendations

### Immediate Security Improvements
1. **Code Signing**: Sign the macOS app bundle with Developer ID
2. **Notarization**: Submit to Apple for malware scanning
3. **Checksums**: Provide SHA256 hashes for releases
4. **Minimal Permissions**: Request only necessary file access

### Advanced Security (Future)
1. **App Sandboxing**: Enable macOS app sandbox
2. **Entitlements**: Explicit permission declarations
3. **Update Mechanism**: Secure auto-updater
4. **Vulnerability Scanning**: Regular dependency audits

## Security Contact

If you discover a security issue, please report it privately.

- Preferred: GitHub Security Advisories (private disclosure)
- Alternative: create a private issue and include reproduction steps

## Threat Model

**Assets to Protect:**
- User's personal files and data
- AI conversation history
- System integrity

**Threat Actors:**
- Malicious installers/updates
- Local privilege escalation
- Information disclosure

**Attack Vectors:**
- Supply chain compromise
- Local file system access
- Social engineering during install

## Conclusion

Little Helper has a **medium security risk profile** suitable for personal use:
- ✅ Strong privacy (local processing)
- ⚠️ Installation requires trust in build/install process
- ✅ Runtime security adequate for intended use case
- 🔄 Additional hardening recommended before wide distribution
