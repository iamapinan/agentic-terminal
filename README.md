# AGENTIC TERMINAL & GUI (v0.2.0)

A high-performance, dark-themed AI Agent Copilot workstation in pure Rust with both a **Native Graphical Desktop Window (GUI)** and a **Terminal UI (TUI)**. Ultra-fast, minimal memory footprint (< 20MB RAM), powered by `eframe`/`egui`, `ratatui`, and `tokio`.

---

## Highlights

- **Native Graphical User Interface (GUI)**:
  - High-DPI native macOS desktop window with cyber-dark aesthetics.
  - Split workspace (interactive shell runner, file chips) and Agent copilot chat.
  - Interactive approval banners for agent workspace control.
- **Automatic Model Loading**:
  - Automatically fetches live model catalogs from selected provider APIs in real-time (OpenRouter `/models`, OpenAI `/models`, Google Gemini `/v1beta/models`, Anthropic Claude, Cloudflare Workers AI).
- **Live Search & Filter**:
  - Instant live filter bar: type keywords (e.g. `claude`, `deepseek`, `flash`, `llama`) to immediately search and select models from hundreds of available choices.
- **Clean Interface Without Emoji**:
  - Clean technical typography and badge labels (`[APP]`, `[DIR]`, `[AGENT]`, `[CONFIRM]`, `[AUTO-EXEC]`, `[READ-ONLY]`).
- **Full macOS App Installation**:
  - `Agentic GUI.app` (Spotlight / Dock desktop app)
  - `Agentic Terminal.app` (Terminal TUI launcher)
  - `agentic-gui` & `agentic-terminal` CLI commands

---

## 5 Supported AI Providers

1. **OpenRouter API** (`https://openrouter.ai/api/v1`) — Live discovery of 100+ models
2. **Google Gemini API** (`gemini-2.5-flash`, `gemini-2.5-pro`, `gemini-2.0-flash`...)
3. **ChatGPT / OpenAI Compatible API** (OpenAI, Groq, Ollama, DeepSeek with custom Base URL)
4. **Anthropic Claude API** (`claude-3-7-sonnet`, `claude-3-5-sonnet`, `claude-3-5-haiku`...)
5. **Cloudflare Workers AI API** (`@cf/meta/llama-3.3-70b-instruct`, Qwen, Mistral...)

---

## Installation & Running

```bash
# Build and install to ~/Applications and ~/.local/bin
./install.sh
```

### Launch Options
- **Spotlight / Launchpad**: Search and open **Agentic GUI** or **Agentic Terminal**.
- **CLI GUI**: Run `agentic-gui`
- **CLI TUI**: Run `agentic-terminal`
