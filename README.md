Language Option / 语言选项：

**English** | [简体中文](README-SC.md)

---

# WinCapture-MCP

WinCapture-MCP is an MCP server tool developed in Rust and designed for Windows. It allows MCP clients to capture and view screenshots of specified windows.

This tool can capture windows even when they are covered by other windows, but it cannot capture minimized windows.

---

## Usage

### Standard Installation

1. Download the archive from the [Releases page](https://github.com/DamoyY/WinCapture-MCP/releases/latest).
2. Extract the executable to any location and remember its path.
3. Add this MCP server to your client.

#### Manual Build

1. Clone or download the source code of this project.
2. Open a terminal in the project root directory, where `Cargo.toml` is located.
3. Run the following command to build the project:

```cmd
cargo build --release
```

After the build is complete, the executable will be generated at `./target/release/WinCapture.exe`.

### MCP Client Configuration

To use this tool with an MCP client, add the server to the client’s configuration file.

#### Examples:

**Claude Code**:

```cmd
claude mcp add WinCapture -- X:/example/path/WinCapture.exe
```

**Codex CLI**：

```
codex mcp add WinCapture -- X:/example/path/WinCapture.exe
```

After completing the configuration, restart the MCP client for the changes to take effect.

---

## Available MCP Tools

This server provides the following two tools to MCP clients:

### `search_hwnd`

*   **Description**: Takes a process name as input and returns a list of matching window HWNDs along with window metadata.
*   **Parameters**:
    *   `process_name` (string): The name of the target process. Matching is case-insensitive, and the `.exe` suffix is optional.
*   **Return Value**: Structured JSON data containing information about all found windows.

### `window_screenshot`

*   **Description**: Takes an HWND as input and returns a screenshot of the corresponding window.
*   **Parameters**:
    *   `hwnd` (string): The handle of the target window. Both hexadecimal and decimal formats are supported.
*   **Return Value**: An image returned as MCP Content with the `image/png` MIME type.

---

## System Requirements

*   **Operating System**: Windows 10 or Windows 11. The system must support the `Windows.Graphics.Capture` API.
*   **Development Environment**: Rust toolchain, 2024 Edition.