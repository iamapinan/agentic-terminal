# AGENTIC TERMINAL TUI (v0.2.0)

A high-performance, dark-themed terminal application written in Rust with an AI agent copilot. Designed for minimal memory footprint (< 16MB RAM) and zero bloat, powered by `ratatui`, `crossterm`, and `tokio`.

---

## What's New in v0.2.0

- **OpenRouter Provider**: Full support for OpenRouter API with popular models (`anthropic/claude-3.7-sonnet`, `meta-llama/llama-3.3-70b-instruct`, `deepseek/deepseek-chat`, etc.).
- **Clean Interface Without Emoji**: Modern technical UI aesthetic using brackets, clean typography, and subtle indicators.
- **Resizable Panels**: Resize chat vs workspace panel by dragging the divider with mouse, or using `[` and `]` (or `Ctrl+Left` / `Ctrl+Right`).
- **Clickable Interface**: Native mouse support! Click on panels to focus, click on buttons (`[Change Model]`, `[Toggle Permission]`, `[Setup]`, `[Approve]`, `[Reject]`), and mouse wheel scrolling.
- **Install as an App**: Packaged as a native macOS Application (`Agentic Terminal.app`) and CLI command (`agentic-terminal`).

---

## Features

- **Ultra Low Memory Footprint**: Single static binary (~5MB release), asynchronous non-blocking event loop, RAM usage < 16MB.
- **Terminal Workspace (Left Panel)**:
  - Real-time directory file list chips.
  - Interactive shell logs (stdout / stderr / return codes).
  - Built-in shell runner supporting `cd` directory tracking and asynchronous command execution.
- **AI Agent Copilot (Right Panel)**:
  - Multi-turn conversation memory with markdown code blocks.
  - Agent can control and inspect the terminal workspace by suggesting or executing commands.
- **Agent Workspace Permissions (Chat Bottom Panel)**:
  - `[CONFIRM]`: Agent proposes a command; press `[Enter]` or click `[APPROVE]` to run.
  - `[AUTO-EXEC]`: Agent automatically runs shell commands in the workspace and feeds execution output back into the conversation.
  - `[READ-ONLY]`: Agent is barred from executing any workspace command.
- **5 Major LLM Providers Supported**:
  1. **Google Gemini API** (`gemini-2.5-flash`, `gemini-2.5-pro`, `gemini-2.0-flash`, `gemini-1.5-flash`)
  2. **ChatGPT / OpenAI Compatible API** (OpenAI, Groq, Ollama, DeepSeek with custom Base URL)
  3. **Anthropic Claude API** (`claude-3-7-sonnet-latest`, `claude-3-5-sonnet-latest`, `claude-3-5-haiku-latest`)
  4. **Cloudflare Workers AI API** (`@cf/meta/llama-3.3-70b-instruct`, `@cf/meta/llama-3.1-8b-instruct`, etc.)
  5. **OpenRouter API** (`anthropic/claude-3.7-sonnet`, `meta-llama/llama-3.3-70b-instruct`, `deepseek/deepseek-chat`, etc.)

---

## Controls & Shortcuts

| Action | Mouse | Keyboard |
|---|---|---|
| Switch Panel Focus | Click on panel | `Tab` |
| Resize Panels | Drag the vertical divider column `│` | `[` (narrower) / `]` (wider) or `Ctrl+Left` / `Ctrl+Right` |
| Scroll Logs / Chat | Mouse scroll wheel | `PageUp` / `PageDown` |
| Provider Settings | Click `[^P Setup]` in header | `Ctrl+P` |
| Switch Model | Click `[Change]` in bottom bar | `Ctrl+M` or `F2` |
| Toggle Permission | Click `[Toggle]` in bottom bar | `Ctrl+X` or `F3` |
| Approve / Reject | Click `[APPROVE]` / `[REJECT]` | `Enter` / `Esc` |
| Clear Logs | — | `Ctrl+L` |
| Quit App | Click `[^Q Exit]` | `Ctrl+C` or `Ctrl+Q` |

---

## Installation as a macOS App

Run the installer script:

```bash
cd /Users/apinan/.gemini/antigravity/scratch/rust-agent-tui
./install.sh
```

This installs:
1. **macOS App**: `~/Applications/Agentic Terminal.app` (launch from Spotlight, Dock, or Finder).
2. **Terminal CLI**: `~/.local/bin/agentic-terminal` (run `agentic-terminal` anywhere in terminal).
