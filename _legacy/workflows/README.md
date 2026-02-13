# Flexible Agent Workflows

This directory contains the "brains" of the new Little Helper. Unlike the previous Rust-based implementation where logic was compiled into the binary, these workflows are dynamic, easy to edit, and can use the full power of Python and modern LLMs.

## Philosophy

1.  **Workflows, not Runtimes**: We don't try to build a complex generic agent runtime. We build specific workflows for specific tasks (Fixing WiFi, Researching, etc.).
2.  **Standard Tools**: We use standard Python libraries (`psutil`, `requests`) instead of reinventing the wheel.
3.  **LLM as Orchestrator**: The Python scripts handle the "doing", the LLM handles the "deciding" and "explaining".

## Available Workflows

### 1. Fix System (`fix_system.py`)
Diagnoses system health using `psutil` and uses an LLM to explain issues in plain English.
- **Usage**: `python3 workflows/fix_system.py`
- **Capabilities**: CPU/Memory checks, Disk space, Zombie processes.

### 2. Research (`research.py`)
(Coming Soon) Smart web research outputting markdown summaries.

### 3. Build (`build.py`)
(Coming Soon) Scaffolding projects based on user specs.
