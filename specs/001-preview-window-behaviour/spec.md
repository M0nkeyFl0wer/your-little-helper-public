# Feature Specification: Interactive Preview Companion

**Feature Branch**: `001-preview-window-behaviour`
**Created**: 2026-01-04
**Status**: Draft
**Input**: Persistent preview window that displays contextual content, web search results, supports interaction (click-to-open, fullscreen, zoom, scroll), includes personality elements like ASCII art, provides mode-specific introductions with specialized agents per mode, and maintains persistent conversation memory

## Clarifications

### Session 2026-01-04

- Q: What is the conversation data retention policy? → A: Unlimited retention - keep all conversations forever until user manually clears. Future archiving via Google Drive or rsync if storage becomes an issue.
- Q: How should the system handle crash recovery? → A: Auto-save each message immediately - no data loss on crash.
- Q: What accessibility requirements apply? → A: Basic accessibility - keyboard navigation, proper labels for screen readers.
- Q: Should users be able to export conversation history? → A: No export for MVP - users can manually access local storage if needed.
- Q: Should local conversation data be encrypted? → A: No app-level encryption - rely on OS file permissions. Users needing encryption can ask the agent to help encrypt files/folders via CLI tools.
- Q: How should terminal access permissions and dependencies be handled? → A: Onboarding flow must (1) get explicit user consent for terminal/command execution, (2) check and install required dependencies (e.g., WSL on Windows), and (3) configure agents to know they have terminal access.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Mode Introduction & Specialized Agents (Priority: P1)

A user clicks on the "Data" tab. The preview panel immediately shows a friendly introduction to Data mode - what it can do, example questions to ask, and the personality of the Data helper. The chat panel clears and starts a fresh conversation with a specialized Data agent who knows how to work with CSV files, databases, and data analysis. Each mode has its own distinct helper with appropriate skills.

**Why this priority**: This defines the core experience of Little Helper as having specialized helpers for each task type. It makes the app feel like a team of friendly experts rather than a generic chatbot.

**Independent Test**: Can be tested by switching to each mode, verifying the introduction appears, and confirming the agent responds with mode-appropriate knowledge and personality.

**Acceptance Scenarios**:

1. **Given** the user is in any mode, **When** they click a different mode tab, **Then** the preview shows an introduction to that mode's capabilities
2. **Given** the user switches to a new mode, **When** the mode loads, **Then** the chat panel starts a fresh conversation (previous mode's conversation is preserved separately)
3. **Given** the user is in Find mode, **When** they ask to search for files, **Then** the Find agent responds with file-search expertise and a helpful personality
4. **Given** the user is in Research mode, **When** they ask a research question, **Then** the Research agent performs web searches and cites sources
5. **Given** the user switches back to a previous mode, **When** the mode loads, **Then** their previous conversation in that mode is restored

---

### User Story 2 - Persistent Conversation Memory (Priority: P1)

A user has been working with the Data helper on analyzing a CSV file. They close Little Helper for the day. When they reopen the app the next day and return to Data mode, they can scroll back through their previous conversation and the Data helper remembers the context - "Last time we were looking at the sales report. Want to continue that analysis?"

**Why this priority**: Memory creates relationship and trust. Users shouldn't have to re-explain everything each session. The helper should feel like a colleague who remembers your work together.

**Independent Test**: Can be tested by having a conversation, closing the app, reopening, and verifying both scrollback history and agent memory of previous context.

**Acceptance Scenarios**:

1. **Given** the user had a conversation yesterday, **When** they reopen the app today, **Then** they can scroll back through the previous conversation history
2. **Given** previous conversations exist, **When** the user asks a follow-up question, **Then** the agent can reference and build on previous discussions
3. **Given** the user is scrolling through history, **When** they reach older messages, **Then** older conversations load seamlessly (pagination/lazy loading)
4. **Given** conversations are stored, **When** the user wants to start fresh, **Then** they can clear history for a mode or start a "new thread"
5. **Given** multiple conversation threads exist in a mode, **When** the user wants to revisit an old topic, **Then** they can browse and select previous conversation threads

---

### User Story 3 - Contextual Preview During Research (Priority: P1)

A user asks Little Helper to research a topic. As the AI searches the web and gathers information, the preview panel shows relevant content: website screenshots, key images, article excerpts, or data visualizations. The user can see where the information is coming from while reading the AI's response, building trust and understanding.

**Why this priority**: This is the core value proposition - the preview panel becomes an active companion that shows context, not just a passive file viewer. Users feel more confident when they can see sources.

**Independent Test**: Can be tested by asking a research question, verifying the preview shows relevant web content (site preview, images), and confirming it persists through the conversation.

**Acceptance Scenarios**:

1. **Given** the user asks a research question, **When** the AI performs a web search, **Then** the preview panel shows a preview of the source website or key image from the search results
2. **Given** multiple sources are found, **When** the AI cites different sources, **Then** the preview can cycle through or show the most relevant source
3. **Given** a web preview is displayed, **When** the user clicks on it, **Then** the full website opens in their default browser

---

### User Story 4 - File Preview with Quick Actions (Priority: P1)

A user asks Little Helper to find a document. The AI locates it and shows it in the preview panel. The user can interact with the preview: zoom in to read details, scroll around large documents, expand to fullscreen for better viewing, or click to open the file in its native application.

**Why this priority**: Equal to Story 3 because file interaction is a core use case. Users need to actually work with files, not just see thumbnails.

**Independent Test**: Can be tested by opening any file, using zoom/scroll controls, clicking fullscreen, and clicking to open in native app.

**Acceptance Scenarios**:

1. **Given** a file is displayed in preview, **When** the user clicks the "open in app" button, **Then** the file opens in the system's default application for that file type
2. **Given** a file is displayed in preview, **When** the user clicks the "show in Finder/Explorer" button, **Then** the file's containing folder opens with the file selected
3. **Given** an image or document is in preview, **When** the user uses zoom controls (buttons or scroll wheel), **Then** the content zooms in or out smoothly
4. **Given** zoomed content exceeds the preview area, **When** the user drags or scrolls, **Then** the view pans to show different parts of the content
5. **Given** any content is in preview, **When** the user clicks the fullscreen button, **Then** the preview expands to fill the screen with an easy way to exit

---

### User Story 5 - Friendly Personality with ASCII Art (Priority: P2)

When Little Helper is thinking, waiting, or celebrating a completed task, the preview panel can display ASCII art that matches the mood - a thinking face, a happy helper character, celebratory graphics. This adds personality and makes the tool feel friendly rather than clinical.

**Why this priority**: Important for the emotional experience but not blocking core functionality. Can be implemented after the interactive features work.

**Independent Test**: Can be tested by triggering different states (thinking, success, error) and verifying appropriate ASCII art displays.

**Acceptance Scenarios**:

1. **Given** the AI is processing a request, **When** the preview has no specific content to show, **Then** it can display a friendly "thinking" ASCII animation or illustration
2. **Given** a task completes successfully, **When** the AI reports success, **Then** the preview can show celebratory ASCII art briefly
3. **Given** an error occurs, **When** the AI reports the problem, **Then** the preview can show a sympathetic/helpful ASCII illustration
4. **Given** the app starts fresh, **When** the preview panel is empty, **Then** it shows a welcoming ASCII art greeting rather than a blank space

---

### User Story 6 - Preview Persists During Conversation (Priority: P2)

While working within a mode, the preview stays visible as the user sends messages and receives responses. Content only changes when the AI explicitly shows new content or the user switches modes.

**Why this priority**: Users need visual stability while working. The preview shouldn't flicker or disappear unexpectedly.

**Independent Test**: Can be tested by opening a file, sending multiple chat messages, and verifying the preview remains stable.

**Acceptance Scenarios**:

1. **Given** a file is open in the preview panel, **When** the user sends a new chat message, **Then** the preview panel remains visible with the same file displayed
2. **Given** a file is open in the preview panel, **When** the AI responds to a message, **Then** the preview panel remains visible with the same file displayed
3. **Given** a file is open in the preview panel, **When** the AI opens a different file, **Then** the preview updates to show the new file (intentional replacement)

---

### User Story 7 - Explicit Preview Control (Priority: P3)

A user wants to maximize chat space or hide the preview. They can close the preview panel, which stays hidden until new content is requested or they manually re-open it.

**Why this priority**: Users need control, but this is lower priority than the core interactive features.

**Independent Test**: Can be tested by closing the panel and verifying it stays closed until explicitly re-opened or new content triggers it.

**Acceptance Scenarios**:

1. **Given** any content is in preview, **When** the user clicks the close button, **Then** the preview panel hides and chat expands
2. **Given** the preview is hidden, **When** the AI has new content to show, **Then** the preview re-appears automatically
3. **Given** the preview is hidden, **When** the user clicks a "show preview" toggle, **Then** the preview re-appears with its last content

---

### Edge Cases

- What happens when a website preview fails to load? Show a placeholder with the URL and option to open in browser
- What happens when the user zooms very far in/out? Set reasonable min/max zoom limits (25% to 400%)
- What happens with very large files in fullscreen? Same zoom/scroll controls work, with performance optimization for large content
- What happens when the previewed file is deleted? Show error state with option to close
- What happens when clicking "open in app" fails? Show friendly error message suggesting the user check file associations
- What happens with ASCII art in dark/light themes? ASCII art adapts to current theme colors
- What happens when switching modes rapidly? Each mode change shows its introduction; conversation switches cleanly
- What happens to unsent messages when switching modes? Unsent text in the input field is preserved per mode
- What happens when conversation history gets very long? Older messages are loaded on-demand as user scrolls up
- What happens when storage is full? Warn user and offer to clear old conversations
- What happens if app crashes mid-conversation? All sent/received messages are already saved; no data loss
- What happens if required dependencies (e.g., WSL) can't be installed? Show clear error with manual installation instructions; allow limited functionality without terminal access
- What happens if user denies terminal permission? Agents work in limited mode (chat-only, no command execution); user can grant permission later in settings

## Requirements *(mandatory)*

### Functional Requirements

**Mode-Specific Agents & Introductions**
- **FR-001**: System MUST display a mode introduction in the preview panel when user switches to a different mode
- **FR-002**: Each mode introduction MUST explain what that mode can do with example questions
- **FR-003**: System MUST start a fresh conversation when user switches to a mode for the first time in a session
- **FR-004**: System MUST preserve separate conversation histories for each mode
- **FR-005**: System MUST restore the previous conversation when user returns to a mode they've already used
- **FR-006**: Each mode MUST have a specialized agent with appropriate knowledge, skills, and personality
- **FR-007**: Mode agents MUST have distinct personalities that match their purpose (e.g., Find is efficient, Research is thorough, Fix is patient)

**Mode Definitions**
- **FR-008**: Find mode agent MUST specialize in file search, location, and organization
- **FR-009**: Fix mode agent MUST specialize in troubleshooting, debugging, and tech support
- **FR-010**: Research mode agent MUST specialize in web search, information gathering, and source citation
- **FR-011**: Data mode agent MUST specialize in CSV, databases, data analysis, and visualization
- **FR-012**: Content mode agent MUST specialize in content creation, writing, and editing

**Conversation Persistence & Memory**
- **FR-013**: System MUST persist conversation histories to local storage with unlimited retention (no automatic deletion; user controls when to clear)
- **FR-013a**: System MUST auto-save each message immediately upon send/receive to prevent data loss on crash
- **FR-014**: Users MUST be able to scroll back through previous conversation history
- **FR-015**: System MUST load older messages on-demand as user scrolls up (lazy loading)
- **FR-016**: Mode agents MUST have access to previous conversation context to provide continuity
- **FR-017**: Users MUST be able to start a new conversation thread within a mode
- **FR-018**: Users MUST be able to browse and select previous conversation threads in a mode
- **FR-019**: Users MUST be able to clear conversation history for a mode
- **FR-020**: System SHOULD include brief memory context (last topic discussed, pending tasks) when agent resumes a conversation after app restart

**Preview Persistence & Context**
- **FR-021**: Preview panel MUST remain visible when user sends chat messages within a mode
- **FR-022**: Preview panel MUST remain visible when AI generates responses within a mode
- **FR-023**: AI MUST be able to update preview content by specifying new content to display

**Web Search Integration**
- **FR-024**: System MUST be able to display website previews when AI performs web searches, using fallback chain: (1) screenshot via wkhtmltoimage if available, (2) Open Graph metadata with og:image, (3) title + snippet + URL text display
- **FR-025**: System MUST be able to display key images from web search results
- **FR-026**: Web previews MUST include the source URL visibly displayed

**Click-to-Open Actions**
- **FR-027**: Users MUST be able to click to open previewed files in their default system application
- **FR-028**: Users MUST be able to click to reveal previewed files in Finder (macOS) or File Explorer (Windows)
- **FR-029**: Users MUST be able to click web previews to open the full URL in their default browser

**Zoom, Scroll, Fullscreen**
- **FR-030**: Users MUST be able to zoom in and out on preview content
- **FR-031**: Users MUST be able to scroll/pan around zoomed content
- **FR-032**: Users MUST be able to expand preview to fullscreen mode
- **FR-033**: Fullscreen mode MUST provide an obvious way to exit (button, Escape key)
- **FR-034**: Zoom and scroll state SHOULD persist while viewing the same content

**Personality & ASCII Art**
- **FR-035**: Preview panel MUST display welcoming content when empty (not blank)
- **FR-036**: System SHOULD display contextual ASCII art during thinking/processing states
- **FR-037**: System SHOULD display celebratory ASCII art on task completion
- **FR-038**: ASCII art MUST be readable in both light and dark themes
- **FR-039**: Each mode MAY have its own ASCII art character/mascot that appears in introductions

**User Control**
- **FR-040**: Users MUST be able to close the preview panel
- **FR-041**: Preview panel MUST re-open when new content is available after being closed
- **FR-042**: System MUST display the current content source (filename, URL, or mode name) in the preview header

**Accessibility**
- **FR-043**: All interactive elements MUST be navigable via keyboard (Tab, Enter, Escape)
- **FR-044**: All buttons, controls, and interactive elements MUST have proper labels for screen readers
- **FR-045**: Focus states MUST be visually distinguishable for keyboard navigation

**Onboarding & Permissions**
- **FR-046**: First-run onboarding MUST request explicit user consent for terminal/command execution capabilities
- **FR-047**: Onboarding MUST check for required system dependencies (e.g., WSL on Windows) and guide installation if missing
- **FR-048**: Onboarding MUST verify terminal access is working before completing setup
- **FR-049**: Agent system prompts MUST explicitly inform each agent of its terminal execution capabilities
- **FR-050**: System MUST display which permissions are granted in settings (terminal access, file access, web access)

### Key Entities

- **Mode**: One of the five specialized contexts (Find, Fix, Research, Data, Content) with its own agent, conversation, and introduction
- **Mode Agent**: A specialized AI personality with specific knowledge, skills, and tone appropriate to its mode
- **Mode Introduction**: Preview content shown when entering a mode, explaining capabilities and showing personality
- **Conversation Thread**: A single conversation within a mode, with its own history and context
- **Conversation History**: All messages in a thread, persisted to storage and loadable on-demand
- **Preview Content**: What is being displayed (file, web page, image, ASCII art, mode introduction) with its source identifier
- **Preview State**: Current display settings (zoom level, scroll position, fullscreen status, visibility)
- **Preview Action**: Clickable actions available (open in app, show in folder, open URL)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Each of the 5 modes displays a unique introduction within 500ms of switching
- **SC-002**: Users can identify which mode they're in by the agent's personality within 2-3 messages
- **SC-003**: Conversation history is correctly preserved and restored for all 5 modes after app restart
- **SC-004**: Users can scroll back through at least 100 previous messages without noticeable delay
- **SC-005**: Agent demonstrates memory of previous context in 80% of follow-up conversations
- **SC-006**: Users can view web search source previews for 80% of research queries
- **SC-007**: Users can successfully open a previewed file in its native app within 2 clicks
- **SC-008**: Users can successfully reveal a file in Finder/Explorer within 2 clicks
- **SC-009**: Zoom controls allow viewing content from 25% to 400% of original size
- **SC-010**: Users can enter and exit fullscreen mode within 1 click each
- **SC-011**: 90% of first-time users can successfully interact with zoom/scroll/fullscreen controls without instruction
- **SC-012**: ASCII art appears appropriately in at least 3 distinct states (welcome, thinking, success)
- **SC-013**: 95% of users complete onboarding with terminal access enabled on first attempt
- **SC-014**: Agents correctly execute terminal commands when user has granted permission

## Assumptions

- The existing preview infrastructure supports text, images, CSV, JSON, HTML, and PDF files
- Web search functionality exists in the AI agent and can provide URLs and potentially screenshots
- The desktop app framework supports opening URLs in external browsers and files in native apps
- Fullscreen mode is supported by the UI framework
- ASCII art assets will be created as part of implementation (not pre-existing)
- The AI provider supports system prompts that can define agent personality and capabilities
- Local storage is available for persisting conversation histories (filesystem or embedded database)
- The AI provider supports context/memory features or we can provide conversation history in prompts
- OS-level file permissions provide adequate data protection; no app-level encryption required for MVP

## Out of Scope

- Video playback in preview
- Audio playback in preview
- Editing files directly in preview (view-only)
- Saving modified zoom/scroll preferences between sessions
- External file change detection (live reload)
- Cross-mode conversation continuity (each mode is independent)
- Custom user-defined modes
- Cloud sync of conversation history (local storage only)
- Sharing conversations with other users
- Conversation export feature (users can manually access local storage if needed)
