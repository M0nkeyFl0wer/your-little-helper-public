# Fix/Secure Mode Preview Integration

## Overview

Transform the preview panel into a **visual security dashboard** when in Fix mode. Instead of just showing text results in chat, display scannable, actionable security information that normal people can understand at a glance.

## Design Principles

1. **Visual over textual** - Show status with colors/icons, not paragraphs
2. **Glanceable** - User should understand their security status in 2 seconds
3. **Actionable** - Every issue has a "Fix it" button
4. **No jargon** - Labels a 12-year-old would understand
5. **Progressive disclosure** - Simple summary first, details on click

---

## Preview Content Types for Fix Mode

### 1. Security Health Dashboard

**When:** User asks "Is my computer safe?" or "Run a security check"

```
┌─────────────────────────────────────────┐
│  🛡️  YOUR COMPUTER'S HEALTH            │
├─────────────────────────────────────────┤
│                                         │
│  ✅ Protection     ON                   │
│  ✅ Updates        All current          │
│  ⚠️  Privacy       3 apps need review   │
│  ✅ Storage        Healthy (45% free)   │
│  ✅ Speed          Running well         │
│                                         │
├─────────────────────────────────────────┤
│  Overall: Looking good! 1 thing to check│
│                                         │
│  [Review Privacy Settings]              │
└─────────────────────────────────────────┘
```

**Data structure:**
```rust
pub struct HealthDashboard {
    pub protection_status: Status,      // ON/OFF/PARTIAL
    pub update_status: Status,          // CURRENT/UPDATES_AVAILABLE/CRITICAL
    pub privacy_issues: Vec<PrivacyIssue>,
    pub storage_percent_free: u8,
    pub performance_status: Status,
    pub overall_score: u8,              // 0-100
    pub summary: String,                // "Looking good!" etc.
}

pub enum Status {
    Good,       // ✅ green
    Warning,    // ⚠️ yellow
    Critical,   // 🔴 red
    Unknown,    // ❓ gray
}
```

---

### 2. Privacy Audit View

**When:** User asks "Can apps spy on me?" or "Check my privacy"

```
┌─────────────────────────────────────────┐
│  🔒  WHO CAN ACCESS WHAT               │
├─────────────────────────────────────────┤
│                                         │
│  📷 CAMERA                              │
│  ├─ Zoom           ✅ You approved      │
│  ├─ Chrome         ⚠️ Review this       │
│  └─ FaceTime       ✅ You approved      │
│                                         │
│  🎤 MICROPHONE                          │
│  ├─ Zoom           ✅ You approved      │
│  ├─ Slack          ✅ You approved      │
│  └─ Unknown App    🔴 Remove access     │
│                                         │
│  📍 LOCATION                            │
│  ├─ Maps           ✅ You approved      │
│  └─ Weather        ✅ You approved      │
│                                         │
│  📁 FILES & FOLDERS                     │
│  └─ 12 apps have access [Review →]      │
│                                         │
├─────────────────────────────────────────┤
│  [Open Privacy Settings]                │
└─────────────────────────────────────────┘
```

**Data structure:**
```rust
pub struct PrivacyAudit {
    pub camera_access: Vec<AppAccess>,
    pub microphone_access: Vec<AppAccess>,
    pub location_access: Vec<AppAccess>,
    pub files_access: Vec<AppAccess>,
    pub contacts_access: Vec<AppAccess>,
}

pub struct AppAccess {
    pub app_name: String,
    pub app_icon: Option<PathBuf>,
    pub status: AccessStatus,
    pub last_used: Option<DateTime>,
}

pub enum AccessStatus {
    Approved,       // User explicitly granted
    NeedsReview,    // Granted but suspicious/unused
    Revoke,         // Definitely should remove
}
```

---

### 3. Suspicious Activity View

**When:** User asks "Is anything sketchy running?"

```
┌─────────────────────────────────────────┐
│  🔍  WHAT'S RUNNING ON YOUR COMPUTER   │
├─────────────────────────────────────────┤
│                                         │
│  ✅ LOOKS NORMAL (47 programs)          │
│                                         │
│  ⚠️  WORTH CHECKING (2 programs)        │
│  ┌─────────────────────────────────────┐│
│  │ "Helper64.exe"                      ││
│  │ Running in background               ││
│  │ Using: 12% CPU, 45MB memory         ││
│  │ [What is this?] [Stop it]           ││
│  └─────────────────────────────────────┘│
│  ┌─────────────────────────────────────┐│
│  │ "node" (12 instances)               ││
│  │ Developer tools - probably fine     ││
│  │ Using: 8% CPU, 890MB memory         ││
│  │ [More info]                         ││
│  └─────────────────────────────────────┘│
│                                         │
│  ✅ NO KNOWN MALWARE DETECTED           │
│                                         │
└─────────────────────────────────────────┘
```

**Data structure:**
```rust
pub struct ProcessAudit {
    pub normal_count: usize,
    pub suspicious: Vec<SuspiciousProcess>,
    pub malware_detected: bool,
    pub scan_time: DateTime,
}

pub struct SuspiciousProcess {
    pub name: String,
    pub pid: u32,
    pub reason: String,           // Why it's flagged
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub recommendation: String,   // "Probably fine" / "Stop this"
}
```

---

### 4. Update Status View

**When:** User asks "Am I protected?" or "Check for updates"

```
┌─────────────────────────────────────────┐
│  🔄  SOFTWARE UPDATES                   │
├─────────────────────────────────────────┤
│                                         │
│  ⚠️  2 SECURITY UPDATES AVAILABLE       │
│                                         │
│  ┌─────────────────────────────────────┐│
│  │ 🍎 macOS Sonoma 14.3.1              ││
│  │ Security fixes - important!         ││
│  │ [Install Tonight]                   ││
│  └─────────────────────────────────────┘│
│  ┌─────────────────────────────────────┐│
│  │ 🌐 Chrome 121.0.6167.85             ││
│  │ Security update                     ││
│  │ [Update Now]                        ││
│  └─────────────────────────────────────┘│
│                                         │
│  ✅ 23 apps are up to date              │
│                                         │
│  Last checked: 2 hours ago              │
│  [Check Again]                          │
└─────────────────────────────────────────┘
```

---

### 5. Connection Monitor View

**When:** User asks "Is anyone connecting to my computer?"

```
┌─────────────────────────────────────────┐
│  🌐  NETWORK CONNECTIONS                │
├─────────────────────────────────────────┤
│                                         │
│  ✅ YOUR FIREWALL IS ON                 │
│                                         │
│  ACTIVE CONNECTIONS:                    │
│  ├─ Chrome → google.com      ✅ Normal  │
│  ├─ Slack → slack.com        ✅ Normal  │
│  ├─ Dropbox → dropbox.com    ✅ Normal  │
│  └─ ??? → 45.33.32.156       ⚠️ Unknown │
│                                         │
│  LISTENING FOR CONNECTIONS:             │
│  ├─ Spotify (local only)     ✅ Safe    │
│  └─ AirDrop (local only)     ✅ Safe    │
│                                         │
│  ⚠️ 1 unknown connection                │
│  [Block It] [More Info]                 │
│                                         │
└─────────────────────────────────────────┘
```

---

### 6. Cleanup Recommendations View

**When:** User asks "Help me clean up" or "Why is my computer slow?"

```
┌─────────────────────────────────────────┐
│  🧹  CLEANUP RECOMMENDATIONS            │
├─────────────────────────────────────────┤
│                                         │
│  SAFE TO REMOVE:                        │
│                                         │
│  ☐ Browser cache         2.3 GB        │
│  ☐ Old downloads         1.8 GB        │
│  ☐ Trash (45 items)      892 MB        │
│  ☐ App caches            654 MB        │
│                                         │
│  ─────────────────────────────          │
│  Total: 5.6 GB you can free up          │
│                                         │
│  [Clean Selected]                       │
│                                         │
├─────────────────────────────────────────┤
│  💡 This won't delete your files,       │
│     just temporary junk.                │
└─────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Data Structures (Week 1)
- [ ] Add `SecurityPreview` variants to `PreviewContent` enum
- [ ] Create data structures for each view type
- [ ] Add platform-specific data gathering functions

### Phase 2: Rendering (Week 1-2)
- [ ] Create `render_health_dashboard()` in preview_panel.rs
- [ ] Create `render_privacy_audit()`
- [ ] Create `render_process_audit()`
- [ ] Create `render_update_status()`
- [ ] Create `render_connection_monitor()`
- [ ] Create `render_cleanup_view()`

### Phase 3: Data Gathering (Week 2-3)
- [ ] macOS: Use `system_profiler`, `lsof`, `ps`, privacy DB queries
- [ ] Windows: Use `wmic`, `netstat`, PowerShell cmdlets
- [ ] Linux: Use `/proc`, `ss`, `systemctl`, package managers

### Phase 4: Actions (Week 3)
- [ ] Wire up "Fix it" buttons to generate approval-required commands
- [ ] Add "Open Settings" actions for each platform
- [ ] Implement cleanup with confirmation

### Phase 5: Polish (Week 4)
- [ ] Add animations for scanning state
- [ ] Add refresh/rescan functionality
- [ ] Add "Last scanned" timestamps
- [ ] Test on all platforms

---

## Chat + Preview Coordination

When the AI runs a security check, the flow is:

1. **User asks:** "Is my computer safe?"

2. **AI responds in chat:**
   > "Let me run a quick health check..."
   >
   > `<command>system_profiler SPSoftwareDataType</command>`
   > `<command>ps aux</command>`
   > (etc.)

3. **User approves commands**

4. **AI processes results, updates preview:**
   > "Good news! Your computer is looking healthy. I found one thing worth checking - 3 apps have camera access you might want to review. Take a look at the security dashboard on the right."

   `<preview type="security" view="health_dashboard">...</preview>`

5. **Preview panel shows:** Visual health dashboard

6. **User clicks "Review Privacy Settings"**

7. **Preview updates to:** Privacy Audit View

8. **User clicks "Remove access" on suspicious app**

9. **AI responds in chat:**
   > "I'll remove camera access for that app."
   >
   > `<command>tccutil reset Camera com.sketchy.app</command>`

10. **User approves, done.**

---

## Success Metrics

- User understands security status within 3 seconds of seeing dashboard
- Zero jargon visible in any view
- Every warning has a one-click fix option
- Users feel MORE secure after enabling terminal access, not less
