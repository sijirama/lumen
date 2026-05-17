## lumen

A desktop chat that can read your Obsidian vault, your Gmail, your calendar, and your screen — and act on them. Built on Gemini.

### Install

Linux / macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/sijirama/lumen/main/install.sh | bash
```

Windows (PowerShell, admin):

```powershell
irm https://raw.githubusercontent.com/sijirama/lumen/main/install.ps1 | iex
```

> Windows scripts are untested. They should work but no promises — open an issue if they don't.

### Uninstall

Linux / macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/sijirama/lumen/main/uninstall.sh | bash
```

Windows (PowerShell, admin):

```powershell
irm https://raw.githubusercontent.com/sijirama/lumen/main/uninstall.ps1 | iex
```

### What it does

- Reads, edits, and writes Obsidian notes — by file, by line, by search.
- Reads Gmail, sends mail, manages Google Calendar and Google Tasks.
- Takes a screenshot when you ask it to.
- Searches the web, your clipboard history, your filesystem.
- Sets reminders that fire as system notifications.
- Remembers things across conversations.

Global hotkey opens the overlay. Default is `Super+L`; configurable in settings.
