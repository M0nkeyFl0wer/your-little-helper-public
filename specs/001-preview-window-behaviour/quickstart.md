# Quickstart: Interactive Preview Companion

**Feature**: 001-preview-window-behaviour
**Date**: 2026-01-04

## Prerequisites

- Rust 1.75+ installed
- Little Helper codebase cloned
- Basic familiarity with egui/eframe

## Getting Started

### 1. Check Out Feature Branch

```bash
cd /home/flower/Projects/little-helper
git checkout 001-preview-window-behaviour
```

### 2. Build and Run

```bash
cargo build
cargo run
```

### 3. Verify Existing Functionality

Before implementing new features, verify:
- [ ] Mode tabs switch correctly (Find, Fix, Research, Data, Content)
- [ ] Sessions persist across restarts
- [ ] File viewer works for basic files
- [ ] Chat sends messages and receives AI responses

## Key Files to Understand

| File | Purpose | Priority |
|------|---------|----------|
| `crates/app/src/main.rs` | Main app, layout, mode switching | HIGH |
| `crates/app/src/sessions.rs` | Session/conversation management | HIGH |
| `crates/viewers/src/lib.rs` | Viewer trait, file type detection | HIGH |
| `crates/agent_host/src/executor.rs` | Command execution, web search | MEDIUM |
| `crates/shared/src/lib.rs` | Shared types, settings | MEDIUM |

## Implementation Order

### Phase 1: Preview Panel Foundation

1. **Create `preview_panel.rs`**
   - Basic panel structure
   - Show/hide toggle
   - File display using existing viewers

2. **Add zoom controls to viewers**
   - Implement `Zoomable` trait
   - Add Ctrl+scroll handling
   - Update image_viewer.rs first (simplest)

### Phase 2: Preview Protocol

3. **Create `preview_types.rs` in shared**
   - Define `PreviewContent` enum
   - Implement tag parsing

4. **Integrate with chat**
   - Parse agent responses for `<preview>` tags
   - Update preview panel on tag detection

### Phase 3: Mode Introductions

5. **Create `ascii_art.rs`**
   - Define ASCII art for each state
   - Theme-aware rendering

6. **Create mode prompts**
   - System prompts per mode
   - Memory integration

### Phase 4: Onboarding

7. **Create `onboarding.rs`**
   - Step wizard UI
   - Dependency detection
   - Permission flow

## Testing Approach

### Manual Testing

```bash
# Run the app
cargo run

# Test mode switching
# - Click each mode tab
# - Verify welcome message changes
# - Verify conversation context switches

# Test preview
# - Open a file in Find mode
# - Verify preview shows file
# - Test zoom controls

# Test persistence
# - Send a few messages
# - Close app
# - Reopen and verify history
```

### Unit Tests

```bash
# Run tests
cargo test

# Run specific crate tests
cargo test -p viewers
cargo test -p app
```

Key test areas:
- Preview tag parsing
- Session save/load
- Zoom clamping (0.25-4.0)
- Mode prompt generation

## Common Tasks

### Adding a New Viewer

1. Create `crates/viewers/src/new_viewer.rs`
2. Implement `Viewer` and `Zoomable` traits
3. Add to `FileType` enum in `lib.rs`
4. Register in viewer factory

### Modifying Mode Prompts

1. Edit `crates/agent_host/src/prompts/MODE.txt`
2. Update personality, examples, tools
3. Test with actual AI provider

### Adding ASCII Art

1. Create art file in `crates/app/src/art/STATE.txt`
2. Register in `ascii_art.rs`
3. Test in light and dark themes

## Architecture Notes

### State Management

- `AppContext` holds global state (in main.rs)
- `SessionManager` manages conversation persistence
- `PreviewState` is runtime-only (not persisted)
- Settings persist to JSON file

### Message Flow

```
User Input → ChatSession → Provider → Response → Parse Tags → Update Preview
                    ↓
              Auto-save to disk
```

### Viewer Loading

```
File path → FileType::from_path() → Create viewer → Load content → Render
```

## Troubleshooting

### Build Errors

```bash
# Clean and rebuild
cargo clean
cargo build

# Check for missing dependencies
cargo check
```

### Session Not Persisting

- Check `~/.config/LittleHelper/sessions/` exists
- Verify write permissions
- Check JSON syntax if manually edited

### Preview Not Updating

- Verify `<preview>` tag syntax in agent response
- Check console for parsing errors
- Ensure panel is visible

## Resources

- [egui Documentation](https://docs.rs/egui)
- [eframe Examples](https://github.com/emilk/egui/tree/master/examples)
- [Spec Document](./spec.md)
- [Data Model](./data-model.md)
- [API Contracts](./contracts/)
