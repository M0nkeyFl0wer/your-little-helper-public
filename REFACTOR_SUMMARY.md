# Little Helper - Plain Text Editor Refactor

**Date:** 2025-12-12
**Status:** ✅ Complete - Ready to Test

---

## What Changed

### Before (Markdown-focused)
- Editor: Markdown text editor
- Preview: Rendered markdown output
- Target: Technical users who know markdown

### After (Non-technical friendly)
- **Editor**: Plain text scratch pad for drafting and notes
- **Preview**: File viewer for HTML, PDF, images, and text files
- **Target**: Non-technical team members (MacAllister Polling)

---

## Key Changes Made

### 1. Removed Markdown Dependencies
- ❌ Removed `comrak` (markdown rendering library)
- ✅ Simplified to plain text editing

### 2. Refactored Editor
**Old:**
```rust
struct EditorState {
    content: String,
    file_path: Option<PathBuf>,
    is_modified: bool,
    rendered_html: String,  // Markdown rendering
}
```

**New:**
```rust
struct EditorState {
    content: String,
    file_path: Option<PathBuf>,
    is_modified: bool,
    // No rendering - just plain text
}
```

### 3. Added File Viewer Preview
**New PreviewContent types:**
- `None` - No file loaded
- `PlainText(String)` - Text files
- `Html(String)` - HTML source (full rendering coming soon)
- `Image(TextureHandle)` - Images (placeholder for now)
- `Pdf(String)` - PDF path (external viewer for now)
- `Error(String)` - Error messages

### 4. Split File Opening
**Two options in File menu:**
1. **Open in Editor...** - For editing text files
2. **Open in Preview...** - For viewing HTML, PDF, images, etc.

### 5. Updated UI Labels
- "Markdown Preview" → "File Viewer"
- "See your markdown rendered" → "View HTML, PDF, images, and files"
- "Write and edit markdown" → "Scratch pad for drafting and notes"

---

## What Works Now

✅ **Plain Text Editor**
- Simple scratch pad interface
- Save/load text files
- No formatting complexity
- Shared with AI for collaboration

✅ **File Viewer**
- Plain text files (display content)
- HTML files (show source code - rendering coming later)
- PDF files (shows path, opens externally)
- Images (placeholder - rendering coming later)

✅ **UI Layout**
- Chat (left) - Always visible with toggle buttons
- Editor (middle) - Collapsible scratch pad
- Preview (right) - Collapsible file viewer
- Both panels have ✖ close buttons

---

## What's Coming Next

### Priority 1: File Search (User Requested)
- Fuzzy file search
- Display results in UI (not terminal)
- Click to open files in editor/preview

### Priority 2: Enhanced File Viewing
- Proper HTML rendering (webview)
- PDF preview (embedded viewer)
- Image thumbnails and display
- Office document preview (if possible)

### Priority 3: Claude API Integration
- Replace Ollama with Claude API
- Plug-and-play team deployment
- Pre-configured API key option

### Priority 4: Team Deployment
- Mac .app packaging
- Code signing
- DMG distribution
- Team documentation

---

## How to Test

```bash
# Run the app
/home/flower/Downloads/claudia-lite/target/release/app

# Try these features:
1. Chat panel (left) - Toggle Editor and Preview buttons
2. Editor (middle) - Type notes, save/load text files
3. Preview (right) - Open HTML/PDF/image files to view
4. File menu - "Open in Editor" vs "Open in Preview"
```

---

## File Locations

```
/home/flower/Downloads/
├── claudia-lite/                        # Track 1: Simple app
│   ├── crates/app/src/main.rs          # Refactored UI code
│   ├── target/release/app               # Built binary
│   ├── MACALLISTER_SPEC.md             # Project spec
│   ├── GOOGLE_DRIVE_STRUCTURE.md       # Google Drive setup
│   ├── SETUP_GUIDE.md                  # Claude API setup
│   └── REFACTOR_SUMMARY.md             # This file
│
└── little-helper-vscodium/              # Track 2: VS Codium fork
    ├── README.md                        # Project overview
    ├── TODO.md                          # Task tracker
    └── little-helper-implementation-plan.md  # Full plan (1,179 lines)
```

---

## Next Steps Discussion

**Questions to answer:**

1. **Which track to prioritize?**
   - Track 1 (simple app) - faster to ship
   - Track 2 (VS Codium) - more features, better long-term
   - Both in parallel?

2. **File search approach?**
   - Fuzzy search library (like `fuzzy-matcher`)
   - Show results in center panel?
   - Click to open in editor or preview?

3. **Claude API setup?**
   - Pre-configured team key?
   - Individual keys per user?
   - Which model (Haiku/Sonnet)?

4. **Mac packaging?**
   - Do you have Mac for building .app?
   - Code signing certificate available?
   - Distribution method (DMG, direct download)?

---

## Summary

✅ **Accomplished:**
- Removed markdown complexity
- Created plain text scratch pad
- Added multi-format file viewer
- Organized UI with collapsible panels
- Set up VS Codium fork project structure

🔧 **Ready for:**
- Fuzzy file search implementation
- Enhanced file viewing (HTML/PDF/images)
- Claude API integration
- Team deployment preparation

---

*Last updated: 2025-12-12*
