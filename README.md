# tLog

A CLI + TUI time-tracking tool written in Rust.

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

# Directory layout

tLog uses the [directories::ProjectDirs](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html) crate
to follow OS conventions for application storage locations.

The default locations are as follows

**Linux**

- `~/.local/share/tlog/` for data
- `~/.config/tlog/` for configuration

**macOS**

TODO: These are probably wrong

- `~/Library/Application Support/tlog/` for data
- `~/Library/Preferences/tlog/` for configuration

**Windows**

- `%AppData%\FrederikNorlyk\tlog\data` for data
- `%AppData%\FrederikNorlyk\tlog\config` for configuration

## Configuration directory

The configuration directory stores a single TOML file called `tlog.toml`, used to control application behavior.

Example:

```toml
time_format = "HoursMinutes"
```

Currently supported settings:

- time_format: controls how durations are displayed in the UI

### Override config directory

You can override the default config location by setting the environment variable `TLOG_CONFIG_DIR`.

## Data directory

The data directory stores the SQLite database used for tracking time entries.

### Override data directory

You can override the default data location by setting the environment variable `TLOG_DATA_DIR`.
