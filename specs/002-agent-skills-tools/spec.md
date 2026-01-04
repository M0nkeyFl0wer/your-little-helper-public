# Feature Specification: Agent Skills and Tools

**Feature Branch**: `002-agent-skills-tools`
**Created**: 2026-01-04
**Status**: Draft
**Input**: User description: "agent skills and tools"
**Depends On**: `001-preview-window-behaviour` (mode agents, preview panel, terminal permissions)

## Overview

Transform Little Helper's mode agents from personality-only assistants into capable tool-users that can execute real actions. Each mode (Find, Fix, Research, Data, Content, **Build**) gains specialized skills appropriate to its domain, enabling agents to perform tasks rather than just discuss them.

**Critical Safety Constraint**: The system operates under a strict "no delete" policy. Agents MUST NEVER delete files. Instead, agents archive, organize, and relocate files to help users manage clutter safely.

**Mode Capability Outlines**: When a user opens any mode tab, the preview window MUST display an outline of that mode's capabilities, available skills, and example use cases.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Fuzzy File Finder with Drive Index (Priority: P1)

A user needs to find a file but can't remember the exact name or location. The Find agent maintains an index of all connected drives and provides fuzzy search (like fzf) so users can locate files by partial matches, keywords, or approximate names.

**Why this priority**: Finding files is the most common user frustration. A fast, fuzzy search that works across all drives solves a daily pain point without requiring users to remember exact file names or paths.

**Independent Test**: Can be fully tested by typing a partial filename like "budget 202" and verifying the agent shows matching files from any drive with instant fuzzy results.

**Acceptance Scenarios**:

1. **Given** I'm in Find mode, **When** I type partial text like "quarterly rep", **Then** the agent shows fuzzy-matched files (quarterly_report.docx, Q3_report_final.pdf, etc.) ranked by relevance
2. **Given** the file index exists, **When** I search for a file, **Then** results appear within 1 second for indexed drives
3. **Given** I found the file I want, **When** I click on it, **Then** I can preview it or reveal it in my file manager
4. **Given** I ask "help me find that spreadsheet from last month about the ocean project", **When** the agent searches, **Then** it uses fuzzy matching on keywords and date ranges to find likely candidates

---

### User Story 2 - Deep Research with Clarifying Questions (Priority: P1)

A user initiates research on a topic. Before beginning, the Research agent asks clarifying questions about audience, desired output format, quality criteria, and examples of what the output should or shouldn't be like. Only after understanding the scope does the agent begin a thorough research process.

**Why this priority**: Research without clear goals wastes time. By front-loading clarification, the agent produces more useful results on the first attempt, respecting the user's time.

**Independent Test**: Can be fully tested by asking "research marine protected areas" and verifying the agent asks clarifying questions before starting, then produces validated research with clickable citations.

**Acceptance Scenarios**:

1. **Given** I'm in Research mode, **When** I ask to research a topic, **Then** the agent first asks: Who is the audience? What output format? What are examples of good/bad output? Any specific criteria?
2. **Given** I've answered the clarifying questions, **When** the agent begins research, **Then** it shows a research plan and estimated time before starting
3. **Given** research is complete, **When** results are displayed, **Then** every claim has a clickable citation linking to the source
4. **Given** I'm researching marine/ocean topics, **When** the agent searches, **Then** it includes the marine protected area training data as foundational context

---

### User Story 3 - Advanced Research with Browser Automation (Priority: P1)

For complex research tasks, the Research agent can use browser automation (Playwright) and Chrome DevTools to gather information from dynamic websites, extract structured data, and capture evidence. Python scripts handle analysis beyond basic search.

**Why this priority**: Modern web content is often dynamic or behind interactions. Browser automation unlocks access to data that simple fetch requests miss.

**Independent Test**: Can be fully tested by asking the agent to extract data from a JavaScript-heavy website and verifying it successfully captures the dynamic content.

**Acceptance Scenarios**:

1. **Given** I request data from a dynamic website, **When** the agent needs to interact with the page, **Then** it uses browser automation to navigate, click, and extract content
2. **Given** browser debugging is needed, **When** the agent encounters issues, **Then** it can inspect network requests and DOM via DevTools
3. **Given** data needs processing beyond basic search, **When** analysis is required, **Then** the agent can execute Python scripts to parse, clean, and summarize findings

---

### User Story 4 - Tech Support with System Diagnostics (Priority: P1)

When a user enters Fix mode, the agent proactively offers to run a basic diagnostic and displays system information (CPU, memory, disk, network) in an htop-like preview. The agent helps troubleshoot network connectivity, performance issues, and browser problems using debugging tools.

**Why this priority**: Users often don't know what information is relevant to their problem. Proactive diagnostics gather context automatically, enabling faster problem resolution.

**Independent Test**: Can be fully tested by switching to Fix mode and verifying the agent offers diagnostics and shows system status in the preview panel.

**Acceptance Scenarios**:

1. **Given** I switch to Fix mode, **When** the mode activates, **Then** the agent offers "Would you like me to run a quick diagnostic?" and shows system overview in preview panel
2. **Given** I describe "my internet is slow", **When** the agent diagnoses, **Then** it runs network tests (ping, DNS, traceroute) and presents results in plain language
3. **Given** I describe a browser issue, **When** the agent investigates, **Then** it can inspect browser state using DevTools MCP to identify problems
4. **Given** I describe "my computer is slow", **When** the agent diagnoses, **Then** it shows CPU, memory, and disk usage with the top resource-consuming processes

---

### User Story 5 - Content Creation with Calendar and MPA Context (Priority: P1)

The Content agent has access to marine protected area training data and references as foundational context. A content calendar is prominently displayed in the preview panel. Clicking the calendar opens it as an interactive spreadsheet (with future sync to Google Sheets).

**Why this priority**: Content work benefits from institutional knowledge and planning visibility. The MPA training data ensures consistent, informed content without re-researching basics.

**Independent Test**: Can be fully tested by switching to Content mode, verifying the calendar appears in preview, clicking to open as spreadsheet, and asking a question that uses MPA context.

**Acceptance Scenarios**:

1. **Given** I switch to Content mode, **When** the preview panel loads, **Then** I see the content calendar prominently displayed
2. **Given** the content calendar is visible, **When** I click on it, **Then** it opens as an interactive spreadsheet view
3. **Given** I ask about marine conservation messaging, **When** the agent responds, **Then** it draws on the MPA training data and references as foundational context
4. **Given** I'm drafting content, **When** the agent helps, **Then** it offers to save versions and track iterations automatically

---

### User Story 6 - Hidden Version Control with Easy Revert (Priority: P1)

All file changes are automatically versioned using a local git-based system that runs silently in the background. Users can easily revert to earlier versions without understanding git. The agent tracks iterations and can show history when asked.

**Why this priority**: Users need safety nets when working with documents. Hidden version control provides protection without requiring technical knowledge, enabling confident iteration.

**Independent Test**: Can be fully tested by making multiple edits to a document, then asking "show me earlier versions" and successfully reverting to a previous state.

**Acceptance Scenarios**:

1. **Given** I edit a file through the agent, **When** changes are saved, **Then** a version is automatically committed with a descriptive message (hidden from user)
2. **Given** I want to see previous versions, **When** I ask "show me earlier versions of this document", **Then** I see a list of versions with dates and descriptions
3. **Given** I select a previous version, **When** I confirm "restore this version", **Then** the file reverts and the current state is also saved as a version
4. **Given** the agent made changes I don't like, **When** I say "undo those changes", **Then** the agent reverts to the pre-change version

---

### User Story 7 - Safe File Organization (No Deletion) (Priority: P1)

The agent NEVER deletes files under any circumstances. When asked to delete, the agent politely refuses and offers to organize instead - archiving files, removing clutter by consolidating redundant copies, or relocating process files to appropriate folders.

**Why this priority**: File deletion is irreversible and high-risk. By making deletion impossible, users can confidently let the agent help organize without fear of accidental data loss.

**Independent Test**: Can be fully tested by asking the agent to delete a file and verifying it refuses but offers organization alternatives.

**Acceptance Scenarios**:

1. **Given** I ask the agent to delete a file, **When** processing the request, **Then** the agent responds: "I can't delete files, but I can help you organize them. Would you like me to archive this or move it to a cleanup folder?"
2. **Given** I ask to "clean up old files", **When** the agent helps, **Then** it identifies candidates, suggests organizing/archiving, and NEVER deletes anything
3. **Given** redundant copies exist, **When** I ask for help, **Then** the agent identifies duplicates and offers to consolidate them into a single location (keeping all copies)
4. **Given** a coding/design error could trigger deletion, **When** any delete operation is attempted, **Then** the system blocks it at the skill level and logs the attempt

---

### User Story 8 - Data Analysis with Validation (Priority: P1)

The Data agent analyzes files with rigorous validation. All data claims include source references. Analysis results in the preview panel have clickable links to the underlying data points. Statistical claims are validated before presentation.

**Why this priority**: Data analysis must be trustworthy. Validation and source linking let users verify claims, building confidence in the agent's analytical capabilities.

**Independent Test**: Can be fully tested by providing a CSV and asking for analysis, then clicking on any statistic to see the underlying data rows.

**Acceptance Scenarios**:

1. **Given** I provide a data file, **When** the agent analyzes it, **Then** every statistic shown has a clickable reference to the source rows
2. **Given** the agent computes a value, **When** displaying results, **Then** it shows the methodology and allows me to drill down into the calculation
3. **Given** data has quality issues, **When** the agent detects them, **Then** it reports validation warnings before presenting analysis

---

### User Story 9 - Dashboard Builder Wizard (Priority: P1)

The Data agent includes a dashboard creation wizard for building survey data dashboards. The wizard guides users step-by-step: analyze data, configure dashboard, aggregate data, validate, and launch. Each step shows progress and offers checkpoints.

**Why this priority**: Dashboard creation is a complex multi-step process. The wizard makes it accessible to non-technical users while ensuring quality at each stage.

**Independent Test**: Can be fully tested by providing survey data and walking through the wizard to produce a working dashboard.

**Acceptance Scenarios**:

1. **Given** I'm in Data mode, **When** I say "build a dashboard" or trigger the wizard, **Then** I see a step-by-step wizard with progress indicator (Step 1/5, 2/5, etc.)
2. **Given** the wizard is at Step 1 (Analyze), **When** I provide survey data, **Then** it profiles the data showing respondent count, question types, and demographics detected
3. **Given** I complete each step, **When** I confirm "continue", **Then** I see a checkpoint confirmation before moving to the next step
4. **Given** validation finds issues, **When** results display, **Then** I can choose to review details, attempt fixes, or proceed with warnings acknowledged

---

### User Story 10 - Build Mode with Spec-Kit Integration (Priority: P1)

A new Build mode integrates with the spec-kit-assistant to help users build software projects using spec-driven development. Users can create specifications, plan implementations, generate tasks, and build features through guided workflows.

**Why this priority**: Software building is a core user need. Integrating spec-kit brings structured project development capabilities into Little Helper.

**Independent Test**: Can be fully tested by asking "start a new feature" and verifying the spec-kit workflow is initiated with clarifying questions.

**Acceptance Scenarios**:

1. **Given** I switch to Build mode, **When** the preview panel loads, **Then** I see Build mode capabilities including: /specify, /plan, /tasks, /implement, /clarify, /analyze
2. **Given** I say "start a new feature called user authentication", **When** the Build agent responds, **Then** it initiates the spec-kit workflow starting with /specify
3. **Given** a specification exists, **When** I ask to plan implementation, **Then** the agent runs the planning workflow and outputs an implementation plan
4. **Given** tasks are generated, **When** I ask to implement, **Then** the agent works through tasks with version control tracking changes

---

### User Story 11 - Mode Capability Outline on Tab Switch (Priority: P1)

When a user switches to any mode tab, the preview window immediately displays an outline of that mode's capabilities, available skills, and example prompts. This helps users understand what each mode can do.

**Why this priority**: Discoverability is critical for adoption. Users need to know what each mode offers without reading documentation.

**Independent Test**: Can be fully tested by switching to each mode and verifying the preview panel shows a capability outline.

**Acceptance Scenarios**:

1. **Given** I switch to any mode (Find, Fix, Research, Data, Content, Build), **When** the tab activates, **Then** the preview panel shows a formatted outline of that mode's capabilities
2. **Given** the capability outline is displayed, **When** I read it, **Then** I see: mode description, available skills, example prompts, and any special features
3. **Given** I'm viewing the capability outline, **When** I click an example prompt, **Then** it populates the input field so I can send it
4. **Given** the mode has special tools (e.g., dashboard wizard for Data), **When** the outline displays, **Then** those tools are prominently featured

---

### User Story 12 - Design Creation with Canva and Gemini (Priority: P1)

The Content agent integrates with Canva MCP for design tools and Nano Banana (Gemini CLI) for AI-generated designs. Users can create, edit, and customize visual content directly through the agent when these services are configured.

**Why this priority**: Visual content creation is a significant productivity boost. Integrating design tools eliminates context-switching between applications.

**Independent Test**: Can be fully tested by asking to create a social media graphic and verifying the Canva integration opens with appropriate templates or Nano Banana generates a design.

**Acceptance Scenarios**:

1. **Given** I'm in Content mode with Canva MCP configured, **When** I ask to create a design, **Then** I see available Canva templates and can start a new design
2. **Given** I have Gemini CLI signed in, **When** I ask for a design from Nano Banana, **Then** the agent generates design options based on my description
3. **Given** I'm working on a design, **When** I want to customize it, **Then** the agent can modify colors, text, and layout through natural language
4. **Given** Canva/Gemini is not configured, **When** I ask for design help, **Then** the agent explains how to set up these integrations

---

### User Story 13 - Persona Engine and Content Automation (Priority: P1)

The Content agent integrates with the persona generation system to create and manage audience personas, and with the MCP research content automation engine for streamlined content workflows. The preview window shows these tools with "coming soon" labels for Meta Ads API and data visualization tools.

**Why this priority**: Understanding audience personas drives effective content. Automation reduces manual effort and ensures consistency across content production.

**Independent Test**: Can be fully tested by asking to generate an audience persona and verifying the persona engine creates detailed audience profiles.

**Acceptance Scenarios**:

1. **Given** I'm in Content mode, **When** I ask to create an audience persona, **Then** the persona engine generates a detailed profile with demographics, interests, and messaging preferences
2. **Given** I have content to create, **When** I trigger content automation, **Then** the MCP engine orchestrates the workflow with proper research and validation
3. **Given** I view the Content mode capability outline, **When** looking at available tools, **Then** I see Meta Ads and dataviz tools marked as "coming soon"
4. **Given** personas are generated, **When** I create content, **Then** the content can be tailored to specific persona segments

---

### Edge Cases

- What happens when a skill times out during execution? (Agent reports timeout, offers to retry or cancel)
- How does the system handle permission changes mid-conversation? (Current conversation respects new permissions immediately)
- What happens if the user's system doesn't support a skill? (Skill shows as unavailable with explanation)
- How are skill errors distinguished from agent errors? (Clear error attribution: "Skill error: file not found" vs "I'm not sure how to help with that")
- What happens when multiple skills need to run in sequence? (Agent chains skills with intermediate feedback visible to user)
- How does the system handle very large result sets? (Results are paginated or summarized with option to see more)
- What if a user insists on deletion? (Agent firmly maintains the no-delete policy and explains the safety rationale)
- What if the file index is out of date? (Agent offers to refresh the index for specific directories)
- What if research takes a long time? (Agent shows progress updates and allows cancellation)
- What if the content calendar isn't set up yet? (Agent offers to create one with a template)

## Requirements *(mandatory)*

### Functional Requirements

#### Core Skill System
- **FR-001**: System MUST provide specialized skills per mode as defined in Mode Skill Sets
- **FR-002**: System MUST execute skills asynchronously without blocking the UI
- **FR-003**: System MUST display skill execution status (running, completed, failed) in real-time
- **FR-004**: Users MUST be able to view all available skills and their current permission status
- **FR-005**: Users MUST be able to modify skill permissions (enable, disable, ask each time)
- **FR-005a**: Sensitive skills MUST require per-session confirmation (user confirms once per app session before skill can execute)
- **FR-006**: System MUST timeout skill executions that exceed reasonable duration (configurable, default 30 seconds for quick skills, 5 minutes for research)
- **FR-006a**: When an external integration fails, System MUST warn the user, offer to help fix or set up the integration, then continue with reduced functionality if user declines assistance
- **FR-006b**: System MUST log detailed execution data for each skill (timing, inputs, outputs, errors) for debugging and troubleshooting
- **FR-006c**: When a command requires sudo, System MUST display a GUI password dialog (not terminal-based) for secure password entry
- **FR-006d**: Users MUST be able to add files or images to agent context via file picker and drag-and-drop functionality

#### File Safety (CRITICAL)
- **FR-007**: System MUST NEVER delete any files under any circumstances - this is a hard block at the skill level
- **FR-008**: System MUST provide archive and organize capabilities as alternatives to deletion
- **FR-009**: When asked to delete, System MUST refuse and offer organization alternatives
- **FR-010**: System MUST log all file operations for audit purposes
- **FR-010a**: Audit logs MUST be accessible only to the primary user via a dedicated settings panel

#### Find Mode
- **FR-011**: System MUST maintain an index of files across all connected drives
- **FR-012**: System MUST provide fuzzy search (fzf-like) that matches partial names and keywords
- **FR-013**: Search results MUST appear within 1 second for indexed content
- **FR-014**: System MUST support search refinement by date, type, size, and location
- **FR-014a**: File index MUST support medium scale (100K-1M files) with sub-second search performance

#### Research Mode
- **FR-015**: Before starting research, System MUST ask clarifying questions about audience, output, criteria, and examples
- **FR-016**: System MUST have access to browser automation (Playwright) for dynamic content
- **FR-017**: System MUST have access to Chrome DevTools MCP for debugging and inspection
- **FR-018**: System MUST support Python script execution for advanced analysis
- **FR-019**: All research claims MUST have clickable citations to sources
- **FR-020**: Research MUST include marine protected area training data as foundational context
- **FR-021**: System MUST validate data and flag questionable sources

#### Fix Mode
- **FR-022**: System MUST proactively offer diagnostics when user enters Fix mode
- **FR-023**: System MUST display system information (CPU, memory, disk, network) in htop-like preview
- **FR-024**: System MUST provide network connectivity troubleshooting tools
- **FR-025**: System MUST provide browser debugging via DevTools MCP

#### Content Mode
- **FR-026**: System MUST include marine protected area training data and references as context
- **FR-027**: System MUST prominently display content calendar in preview panel
- **FR-028**: Clicking content calendar MUST open it as interactive spreadsheet
- **FR-029**: System SHOULD support future sync to Google Sheets (when configured)

#### Version Control
- **FR-030**: System MUST automatically version all file changes using local git
- **FR-031**: Version control MUST be hidden from user (no git UI or terminology)
- **FR-032**: Users MUST be able to view and restore previous versions via natural language
- **FR-033**: System MUST preserve history when reverting (no data loss on revert)

#### Data Mode
- **FR-034**: All data analysis claims MUST have clickable references to source data
- **FR-035**: System MUST validate data quality before presenting analysis
- **FR-036**: System MUST show methodology for computed values
- **FR-037**: System MUST provide dashboard builder wizard for survey data dashboards
- **FR-038**: Dashboard wizard MUST guide users through: analyze, configure, aggregate, validate, launch

#### Build Mode
- **FR-039**: System MUST integrate with spec-kit-assistant at configured path
- **FR-040**: System MUST support spec-driven development workflow: specify → plan → tasks → implement
- **FR-041**: System MUST track implementation progress and version all code changes
- **FR-042**: Build mode MUST display available speckit commands in capability outline

#### Design Tools
- **FR-043**: Content mode SHOULD integrate with Canva MCP when configured
- **FR-044**: Content mode SHOULD support Nano Banana design generation via Gemini CLI when signed in
- **FR-045**: System MUST gracefully handle missing design tool configurations with setup guidance

#### Persona and Content Automation
- **FR-046**: Content mode MUST integrate with persona generation system for audience personas
- **FR-047**: Content mode MUST integrate with MCP research content automation engine
- **FR-048**: Persona profiles MUST include demographics, interests, and messaging preferences
- **FR-049**: Preview window MUST show "coming soon" labels for Meta Ads and dataviz tools
- **FR-050**: Content automation MUST include proper research and validation steps

### Key Entities

- **Skill**: A discrete capability an agent can invoke. Attributes: name, description, mode associations, permission level (safe/sensitive), input schema, output type
- **SkillPermission**: User's grant level for a skill. Attributes: skill_id, permission (enabled/disabled/ask), scope (per-mode or global), granted_at
- **SkillExecution**: A record of a skill being used. Attributes: skill_id, mode, timestamp, input, output, status, duration
- **FileIndex**: Searchable index of files across drives. Attributes: path, name, type, size, modified_date, keywords, drive_id
- **ResearchPlan**: Pre-research clarification. Attributes: topic, audience, output_format, quality_criteria, positive_examples, negative_examples
- **ContentCalendar**: Planning artifact for content work. Attributes: entries (date, title, status, assignee, notes)
- **FileVersion**: A versioned state of a file. Attributes: file_path, version_id, timestamp, description, content_hash
- **MPAContext**: Marine protected area training data. Attributes: topics, references, key_facts, messaging_guidelines
- **DashboardProject**: Dashboard creation workflow state. Attributes: step (1-5), data_profile, config, validation_status, output_path
- **BuildProject**: Spec-kit project state. Attributes: spec_path, plan_path, tasks_path, current_task, implementation_status
- **DesignConfig**: Design tool configuration. Attributes: canva_mcp_enabled, gemini_cli_signed_in, default_templates

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: File searches return results within 1 second for indexed drives
- **SC-002**: Fuzzy search successfully finds files with partial/approximate names in 90% of attempts
- **SC-003**: 100% of research outputs include clickable source citations
- **SC-004**: Research clarifying questions are asked before starting in 100% of research requests
- **SC-005**: System diagnostic appears within 3 seconds of entering Fix mode
- **SC-006**: Content calendar is visible immediately when entering Content mode
- **SC-007**: Users can revert to any previous version within 3 clicks
- **SC-008**: 0% of file deletion operations succeed (hard block)
- **SC-009**: All data analysis claims can be traced to source data via click
- **SC-010**: Version history is available for all agent-modified files
- **SC-011**: Dashboard wizard completes all 5 steps with checkpoints at each stage
- **SC-012**: Build mode successfully invokes spec-kit commands when project path is configured
- **SC-013**: Mode capability outline displays within 1 second of tab switch
- **SC-014**: Design tools gracefully degrade when Canva/Gemini not configured (shows setup guidance)

## Mode Skill Sets

| Mode         | Skill                    | Description                                        | Permission |
|--------------|--------------------------|----------------------------------------------------|------------|
| **Find**     | fuzzy_file_search        | fzf-like search across indexed drives              | Safe       |
| **Find**     | drive_index              | Maintain and update file index                     | Safe       |
| **Find**     | file_preview             | Preview file contents                              | Safe       |
| **Find**     | file_organize            | Move/archive files (never delete)                  | Sensitive  |
| **Fix**      | system_diagnostic        | Run comprehensive system check                     | Safe       |
| **Fix**      | system_info_display      | Show htop-like system status                       | Safe       |
| **Fix**      | network_troubleshoot     | Diagnose connectivity issues                       | Safe       |
| **Fix**      | browser_debug            | Inspect browser via DevTools MCP                   | Sensitive  |
| **Fix**      | log_analysis             | Read and analyze system logs                       | Sensitive  |
| **Research** | research_clarify         | Ask clarifying questions before research           | Safe       |
| **Research** | web_search               | Search the internet                                | Safe       |
| **Research** | web_fetch                | Fetch and extract web content                      | Safe       |
| **Research** | browser_automate         | Use Playwright for dynamic sites                   | Sensitive  |
| **Research** | python_analysis          | Execute Python scripts for analysis                | Sensitive  |
| **Research** | mpa_context              | Access marine protected area training data         | Safe       |
| **Research** | citation_validate        | Validate and format citations                      | Safe       |
| **Data**     | file_read                | Read file contents                                 | Safe       |
| **Data**     | parse_data               | Parse CSV, JSON, Excel formats                     | Safe       |
| **Data**     | analyze_with_references  | Compute stats with source linking                  | Safe       |
| **Data**     | data_validate            | Check data quality and consistency                 | Safe       |
| **Data**     | generate_chart           | Create visualizations                              | Safe       |
| **Content**  | mpa_context              | Access marine protected area training data         | Safe       |
| **Content**  | content_calendar         | View and edit content calendar                     | Safe       |
| **Content**  | calendar_spreadsheet     | Open calendar as interactive spreadsheet           | Safe       |
| **Content**  | file_read                | Read existing documents                            | Safe       |
| **Content**  | file_write_versioned     | Save with automatic versioning                     | Sensitive  |
| **Content**  | grammar_check            | Check spelling and grammar                         | Safe       |
| **Data**     | dashboard_wizard         | Step-by-step dashboard creation workflow           | Safe       |
| **Data**     | dashboard_analyze        | Profile survey data for dashboard building         | Safe       |
| **Data**     | dashboard_config         | Configure dashboard layout and components          | Safe       |
| **Data**     | dashboard_validate       | Run validation gates on dashboard data             | Safe       |
| **Data**     | dashboard_qa             | Interactive QA testing of dashboard                | Safe       |
| **Build**    | speckit_specify          | Create feature spec from description               | Safe       |
| **Build**    | speckit_plan             | Generate implementation plan                       | Safe       |
| **Build**    | speckit_tasks            | Generate task list from spec/plan                  | Safe       |
| **Build**    | speckit_implement        | Execute tasks with tracking                        | Sensitive  |
| **Build**    | speckit_clarify          | Ask clarifying questions for spec                  | Safe       |
| **Build**    | speckit_analyze          | Cross-artifact consistency analysis                | Safe       |
| **Build**    | speckit_constitution     | Create/update project constitution                 | Safe       |
| **Content**  | canva_mcp                | Access Canva design tools via MCP                  | Safe       |
| **Content**  | nano_banana_design       | Generate designs via Gemini CLI                    | Sensitive  |
| **Content**  | design_templates         | Access and customize design templates              | Safe       |
| **Content**  | persona_engine           | Generate and manage audience personas              | Safe       |
| **Content**  | content_automation       | Automated content workflows via MCP engine         | Safe       |
| **Content**  | meta_ads *(coming soon)* | Meta API integration for ad management             | Sensitive  |
| **Content**  | dataviz *(coming soon)*  | Data visualization tools for content              | Safe       |
| **All**      | version_history          | View file version history                          | Safe       |
| **All**      | version_restore          | Restore previous file version                      | Sensitive  |
| **All**      | mode_capability_outline  | Display mode capabilities when tab opens           | Safe       |

## Research Methodology & Formatting Preferences

The Research agent should apply established methodologies based on the research type:

### OSINT (Open Source Intelligence) Tasks
- Use established OSINT frameworks and techniques for information gathering
- Cross-reference multiple sources to validate findings
- Document source provenance and access timestamps
- Apply appropriate caution labels for unverified information
- Respect privacy boundaries and legal constraints

### Academic Research Tasks
- Follow academic citation standards (prefer consistent format, typically APA or Chicago)
- Distinguish between primary and secondary sources
- Note peer-review status of sources
- Include methodology descriptions for any analysis
- Flag potential biases in sources
- Provide literature review context when appropriate

### Data Science Research Tasks
- Document data sources and collection methodology
- Include sample sizes and statistical confidence levels
- Visualize data with appropriate chart types
- Note limitations and caveats of analysis
- Provide reproducibility information (how findings can be verified)

### General Formatting Preferences
- Use markdown formatting for readability
- Include executive summary for longer research outputs
- Organize findings hierarchically (key findings → supporting details)
- All citations must be clickable links to sources
- Include "last verified" dates for time-sensitive information
- Flag any sources that are paywalled or require authentication

## Assumptions

- Marine protected area training data exists and is accessible at a known location
- Local git is available for version control (bundled if necessary)
- Playwright and Chrome DevTools MCP can be installed/configured
- Python runtime is available for script execution
- File indexing can run in background without significant performance impact
- Users have granted filesystem access to the application
- Spec-kit-assistant is available at `/home/flower/Projects/spec-kit-assistant` for Build mode
- Canva MCP is optional; graceful fallback when not configured
- Gemini CLI (Nano Banana) is optional; requires workspace API key for design generation
- Universal Gemini API keys for workspace will be configured via admin panel (future)
- Persona generation system is available at `/home/flower/Projects/persona-generation-system`
- MCP research content automation engine is available at `/home/flower/Projects/MCP-research-content-automation-engine`
- Meta Ads API integration is planned for future release (coming soon)
- Data visualization tools for content are planned for future release (coming soon)

## Clarifications

### Session 2026-01-04
- Q: Who should have access to audit logs of file operations and skill executions? → A: Primary user only via dedicated settings panel
- Q: When an external integration fails, how should the agent respond? → A: Warn user, offer to help fix/set up, then continue with reduced functionality if user declines
- Q: What level of observability for skill execution? → A: Detailed logs per skill execution (timing, inputs, outputs, errors)
- Q: How should Sensitive skills be authorized? → A: Per-session confirmation (confirm once per app session)
- Q: What is the expected scale for the file index? → A: Medium (100K-1M files)
- Note: Sudo commands require GUI password dialog (backend exists, UI not yet built)
- Note: Users need file picker + drag-and-drop for adding context to agents

## Out of Scope

- Custom user-defined skills (future feature)
- Skill marketplace or sharing between users
- Remote skill execution on other machines
- Natural language skill discovery ("what skills do you have?")
- Multi-agent skill coordination
- Google Sheets sync (future, when org drive is set up)
- Cloud backup of versions (local git only for now)
- File deletion of any kind
