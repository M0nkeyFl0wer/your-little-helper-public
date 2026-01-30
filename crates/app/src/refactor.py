#!/usr/bin/env python3
"""
Refactor main.rs by extracting types and utils into separate modules.
This script creates a new main.rs that imports from types.rs and utils.rs
"""

import re

# Read the original main.rs
with open("main.rs", "r") as f:
    content = f.read()

# Lines to remove (type definitions that are now in types.rs)
lines_to_remove = [
    (11, 34),  # AiResult, CommandExecResult, WebPreviewResult
    (55, 96),  # AppScreen, ChatMode, ChatMessage, ActiveViewer
    (97, 148),  # AppState struct
]

# We'll create the new main.rs by:
# 1. Keeping the imports at the top
# 2. Adding mod declarations for types and utils
# 3. Removing the duplicate type definitions
# 4. Updating references to use crate::types:: and crate::utils::

# For now, let's just create a simpler approach - update main.rs to use the modules
# while keeping the types there temporarily

new_content = """use agent_host::{AgentHost, CommandResult};
use eframe::egui;
use parking_lot::Mutex;
use services::web_preview::WebPreviewService;
use shared::agent_api::ChatMessage as ApiChatMessage;
use shared::preview_types::{parse_preview_tags, strip_preview_tags, PreviewContent};
use shared::settings::AppSettings;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, OnceLock};

// Default mascot image (boss's dog!)
const DEFAULT_MASCOT: &[u8] = include_bytes!("../assets/default_mascot.png");

// Module declarations
mod types;
pub use types::*;

mod utils;
pub use utils::*;

mod secrets;
use secrets::{OPENAI_API_KEY, PRELOAD_SKIP_ONBOARDING, PRELOAD_USER_NAME};

mod support_info;
use support_info::{SUPPORT_BUTTON_TEXT, SUPPORT_LINK};

mod ascii_art;
mod onboarding;
mod preview_panel;

mod context;
use context::{get_campaign_summary, load_campaign_context, load_ddd_workflow, load_personas};

"""

# Find where AppState impl starts (around line 150)
appstate_impl_start = content.find("impl Default for AppState {")
if appstate_impl_start == -1:
    print("Could not find AppState impl")
    exit(1)

# Keep everything from AppState impl onwards
rest_of_file = content[appstate_impl_start:]

# Combine
final_content = new_content + rest_of_file

# Write the new main.rs
with open("main.rs", "w") as f:
    f.write(final_content)

print("Updated main.rs to use types and utils modules")
print(f"New file size: {len(final_content)} bytes")
