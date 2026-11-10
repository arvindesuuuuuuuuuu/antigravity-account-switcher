# Antigravity Account Switcher

A fast, cross-platform desktop application and CLI tool for **Linux** and **Windows** designed specifically to manage, monitor, and hot-switch between multiple Google accounts in the **Google Antigravity Standalone Desktop App**.

> [!IMPORTANT]
> **Antigravity ONLY:** This project strictly targets the standalone Antigravity Electron application (`~/.config/Antigravity` on Linux, `%APPDATA%\Antigravity` on Windows, and Keyring `gemini:antigravity`). It deliberately excludes and does not touch `Antigravity IDE`.

When your usage, 5-hour rolling allowances, or weekly request caps run out (e.g. HTTP 429 / `RESOURCE_EXHAUSTED`), the app can automatically failover and hot-swap to your next available Google account seamlessly without disrupting your work.

---

## ✨ Features

- **⚡ Zero-Interruption Hot-Switching:** Antigravity's supervisor architecture decouples the main window from the background `language_server` AI process. When switching accounts, the switcher writes credentials across the system Keyring and fallback files, then terminates the child `language_server`. Antigravity's built-in supervisor automatically respawns the child process within ~2 seconds with the new credentials—retaining your open project, chat history, and active windows without restarting!
- **📊 100% Accurate Live Model Usage Breakdown:** Zero-latency connection to Antigravity's local Language Server RPC (`/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary` and `GetUserStatus`) as well as upstream Google Cloud Code endpoints. Displays the exact model group quotas with 100% fidelity to Antigravity Desktop's "View Usage" popup:
  - **Gemini Models** (Gemini 3.8 Flash High/Medium/Low, Gemini 2.5 Pro): 5-Hour rolling quota & Weekly limits + reset countdown timers.
  - **Claude and GPT models** (Claude Opus 5.5, Claude Sonnet 5.5, GPT-OSS 120B): Third-party model weekly & 5-hour limits.
  - **Gemini 4 Argon** (Gemini 4 Argon Medium/High/Low): Dedicated cutting-edge preview quota tracking.
  - **Account Credits Tracker:** Displays live Google AI Pro available Prompt Credits (e.g. 500) and Flow Credits (e.g. 100).
- **🔄 Auto-Switch Failover Engine:** Monitors `~/.config/Antigravity/logs/language_server.log` (and Windows equivalent) in real time for HTTP 429 / `RESOURCE_EXHAUSTED` errors. When triggered, it marks the exhausted account, starts a countdown cooldown timer, and auto-switches to the next available account.
- **🛡️ Multi-Tier Credential Synchronization:**
  - **Linux:** Secret Service keyring (`gemini:antigravity`), `~/.gemini/jetski-standalone-oauth-token`, `~/.gemini/antigravity-cli/antigravity-oauth-token`, `~/.gemini/oauth_creds.json`, and `~/.config/Antigravity/app_storage.json`.
  - **Windows:** Windows Credential Manager (`gemini:antigravity`), `%USERPROFILE%\.gemini\jetski-standalone-oauth-token`, and `%APPDATA%\Antigravity\app_storage.json`.
- **🖥️ Native Cross-Platform Stack:**
  - Native Linux release binary: `bin/linux/antigravity-account-switcher` (Tauri v2 + Rust + Web UI)
  - Native Windows build scripts: `scripts/build-windows.bat` & `scripts/build-windows.ps1`
  - Zero-dependency Python companion CLI & mini-GUI: `python-companion/antigravity_switcher.py`
- **🔔 System Tray Control:** Hot-switch or trigger auto-switch straight from your OS taskbar or system tray.

---

## 🚀 Quick Start

### 1. Run the Native Desktop App (Linux)

The native Linux binary is compiled and ready in `bin/linux/`:

```bash
./bin/linux/antigravity-account-switcher
```

### 2. Run the Python Companion CLI / GUI

The Python companion works out of the box on both Linux and Windows with standard Python 3:

```bash
# Check current active Antigravity account and status
python3 python-companion/antigravity_switcher.py status

# Save current active account as a profile
python3 python-companion/antigravity_switcher.py save "Personal Account"

# Query and display live 5h & weekly quotas from Google Cloud Code API
python3 python-companion/antigravity_switcher.py quota

# List all saved account profiles
python3 python-companion/antigravity_switcher.py list

# Hot-switch to another account (by email, label, or ID)
python3 python-companion/antigravity_switcher.py switch "work@gmail.com"

# Auto-switch to next available account
python3 python-companion/antigravity_switcher.py next

# Launch the lightweight desktop GUI
python3 python-companion/antigravity_switcher.py gui

# Run the background rate-limit watcher daemon
python3 python-companion/antigravity_switcher.py watch
```

---

## 🛠️ Adding Multiple Google Accounts

1. **Capture your first account:**
   - Launch the Antigravity desktop app and sign in with your primary Google account.
   - Open this switcher and click **"+ Save"** (or run `python3 antigravity_switcher.py save "Personal"`).
2. **Add additional accounts:**
   - **Method A (In-App Sign-in):** In Antigravity, log in with your second Google account. In the switcher, click **"+ Add Account"** -> **"Capture Current Session"** and name it (e.g. `"Work Pro"`).
   - **Method B (Direct Token / JSON):** In the switcher, click **"+ Add Account"** -> **"Enter Token / JSON"** and input the email and OAuth access & refresh tokens.
3. **Switching & Auto-Switching:**
   - Click **"⚡ Switch"** on any profile to hot-reload credentials into Antigravity in ~2 seconds.
   - Or keep the app open in your system tray: when Antigravity exhausts quota (HTTP 429), it automatically switches to your next account!

---

## 📁 Storage Architecture & Paths

| Component | Linux | Windows | Description |
| :--- | :--- | :--- | :--- |
| **Antigravity App Storage** | `~/.config/Antigravity/app_storage.json` | `%APPDATA%\Antigravity\app_storage.json` | Active login username & state |
| **Standalone OAuth Fallback** | `~/.gemini/jetski-standalone-oauth-token` | `%USERPROFILE%\.gemini\jetski-standalone-oauth-token` | Primary fallback token used by language_server |
| **CLI OAuth Fallback** | `~/.gemini/antigravity-cli/antigravity-oauth-token` | `%USERPROFILE%\.gemini\antigravity-cli\antigravity-oauth-token` | Synchronized CLI credentials |
| **Gemini OAuth File** | `~/.gemini/oauth_creds.json` | `%USERPROFILE%\.gemini\oauth_creds.json` | OAuth credentials store |
| **Credential User Scope** | `~/.config/antigravity/credential-user-scope` | `%APPDATA%\antigravity\credential-user-scope` | Antigravity internal scope ID |
| **System Keyring** | Secret Service (`gemini:antigravity`) | Credential Manager (`gemini:antigravity`) | Primary OS-level secret vault |
| **Logs** | `~/.config/Antigravity/logs/language_server.log` | `%APPDATA%\Antigravity\logs\language_server.log` | Monitored for 429 & RESOURCE_EXHAUSTED |
| **Profiles** | `~/.config/antigravity-account-switcher/profiles.json` | `%APPDATA%\antigravity-account-switcher\profiles.json` | Switcher account registry |

---

## 🔨 Building from Source

### Linux
```bash
./scripts/build-linux.sh
```

### Windows
From PowerShell or Command Prompt:
```cmd
scripts\build-windows.bat
```
or
```powershell
.\scripts\build-windows.ps1
```

---

## 🧪 Testing

Run the automated integration test suite:
```bash
python3 python-companion/test_switcher.py
```
Outputs:
```
.....
----------------------------------------------------------------------
Ran 5 tests in 0.005s

OK
```
# antigravity-account-switcher
