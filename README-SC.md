语言选项 / Language Option：

[English](README.md) | **简体中文**

---

# WinCapture-MCP

WinCapture-MCP 是一个基于 Rust 开发的 Model Context Protocol (MCP) 服务端工具，它为 Windows 系统设计，允许 MCP 客户端对指定的窗口进行截图查看。

该工具能够捕获被遮挡的窗口，无法捕获最小化的窗口。

---

## 使用方法

### 一般安装

1. 从[发布页面](https://github.com/DamoyY/WinCapture-MCP/releases/latest)下载压缩包。
2. 解压可执行文件到任意位置，并记住其路径。
3. 在客户端中添加该 MCP。

#### 手动编译

1. 克隆或下载本项目的源代码。
2. 在项目根目录（`Cargo.toml` 所在目录）打开终端。
3. 执行以下命令进行编译：

```cmd
cargo build --release
```

编译完成后，可执行文件将生成在 `./target/release/WinCapture.exe`。

### MCP 客户端配置

要让 MCP 客户端使用此工具，需要在客户端的配置文件中添加该服务器。

#### 示例：

**Claude Code**：

```
claude mcp add WinCapture -- X:/example/path/WinCapture.exe
```

**Codex CLI**：

```
codex mcp add WinCapture -- X:/example/path/WinCapture.exe
```

配置完成后，重启 MCP 客户端即可生效。

---

## 提供的 MCP 工具列表

该服务器向 MCP 客户端提供了以下两个工具：

### `search_hwnd` 工具：

*   **描述**：输入进程名，返回匹配窗口的 HWND 列表及窗口元信息。
*   **参数**：
    *   `process_name` (字符串): 目标进程的名称。不区分大小写，带或不带 `.exe` 后缀均可。
*   **返回值**：结构化的 JSON 数据，包含查找到的所有窗口信息。

### `window_screenshot` 工具：

*   **描述**：输入 HWND，返回该窗口的画面截图。
*   **参数**：
    *   `hwnd` (字符串): 目标窗口的句柄。支持十六进制或十进制格式。
*   **返回值**：一张图片，以 `image/png` 类型的 MCP Content 输出给 AI。

---

## 系统环境要求

*   **操作系统**：Windows 10 或 Windows 11（需要系统支持 `Windows.Graphics.Capture` API）。
*   **开发环境**：Rust 工具链（2024 Edition）。
