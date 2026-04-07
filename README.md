Language Option / 语言选项：

[简体中文](README-SC.md) | English

---

# WinCapture-MCP

WinCapture-MCP is a Model Context Protocol (MCP) server written in Rust for Windows. It allows MCP clients to search for windows by process name and capture screenshots of specific windows by HWND.

This tool can capture windows that are obscured, but it cannot capture minimized windows.

## System Requirements

*   **Operating system**: Windows 10 or Windows 11 (the system must support the `Windows.Graphics.Capture` API).
*   **Development environment**: Rust toolchain (Edition 2024, latest stable Rust recommended).

## Build

1. Clone or download this repository.
2. Open a terminal in the project root directory where `Cargo.toml` is located.
3. Run the following command to build the project:

```bash
cargo build --release
```

After the build completes, the executable will be generated at `target/release/WinCapture.exe`. Record its absolute path so it can be configured in your MCP client.

## MCP Client Configuration

To use this tool from an MCP client such as Claude Code, add the server to the client's configuration.

For example, in Claude Code run:

```bash
claude mcp add WinCapture -- X:/example/path/WinCapture.exe
```

After the configuration is added, restart the MCP client for the change to take effect.

## Available MCP Tools

This server exposes the following two tools to MCP clients:

### 1. search_hwnd

*   **Description**: Accepts a process name and returns a list of matching window HWNDs together with window metadata.
*   **Parameters**:
    *   `process_name` (string): The target process name. Matching is case-insensitive, and both names with or without the `.exe` suffix are accepted, such as `"explorer"` or `"chrome.exe"`.
*   **Returns**: Structured JSON data containing all matched window information.

### 2. window_screenshot

*   **Description**: Accepts an HWND and returns a screenshot of the corresponding window.
*   **Parameters**:
    *   `hwnd` (string): The target window handle. Both hexadecimal formats such as `"0x00000000000A1B2C"` or `"0x2A"` and decimal format are supported.
*   **Returns**: A PNG image returned to the client as MCP content with the `image/png` MIME type, which AI clients can display directly.
