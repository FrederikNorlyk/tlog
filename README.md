# tLog

A CLI + TUI time-tracking tool written in Rust.

[![codecov](https://codecov.io/gh/FrederikNorlyk/tlog/branch/main/graph/badge.svg)](https://codecov.io/gh/FrederikNorlyk/tlog)

## Overview

tLog tracks time per project using a local SQLite database and provides both:

- a command-line interface (CLI)
- a terminal user interface (TUI)

# Installation

## Linux

Run the installation script:

```bash
curl -sSL https://raw.githubusercontent.com/FrederikNorlyk/tlog/main/scripts/linux-install.sh | bash
```

## Windows

Run the installer script with the following one-liner (downloads the installer and runs it):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command 'Invoke-WebRequest "https://raw.githubusercontent.com/FrederikNorlyk/tlog/main/scripts/windows-install.ps1" -OutFile "$env:TEMP\tlog-install.ps1"; & "$env:TEMP\tlog-install.ps1"'
```

## Directory layout

tLog uses the [directories::ProjectDirs](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html) crate
to follow OS conventions for application storage locations.

The default locations are as follows

**Linux**

- `~/.local/share/tlog/` for data
- `~/.config/tlog/` for configuration

**macOS**

- `~/Library/Application Support/com.FrederikNorlyk.tlog` for data and configuration

**Windows**

- `%AppData%\FrederikNorlyk\tlog\data` for data
- `%AppData%\FrederikNorlyk\tlog\config` for configuration

### Configuration directory

The configuration directory stores a single TOML file called `tlog.toml`, used to control application behavior.

Example:

```toml
time_format = "HoursMinutesSeconds"

[opener]
url = "https://www.subdomain.atlassian.net/browse/%s"
desc = "Open in Jira"
```

Supported settings:

- `time_format`: Controls how durations are displayed in the UI. 
  - Supported values:`HoursMinutesSeconds`, `HoursMinutes`, `DecimalHours`, and `Seconds`.
- `opener`: Optional configuration for opening the selected project in a browser using the `o` key.
  - `url`: URL template. The `%s` placeholder is replaced with the name of the selected project.
  - `desc`: Description displayed for the opener.

#### Override config directory

You can override the default config location by setting the environment variable `TLOG_CONFIG_DIR`.

### Data directory

The data directory stores the SQLite database used for tracking time entries.

#### Override data directory

You can override the default data location by setting the environment variable `TLOG_DATA_DIR`.
