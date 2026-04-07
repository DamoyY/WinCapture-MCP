# WinCapture-MCP

WinCapture-MCP 是一个基于 Rust 开发的 Model Context Protocol (MCP) 服务端工具，专为 Windows 系统设计。它允许支持 MCP 的客户端按进程名查询窗口信息，并根据窗口句柄（HWND）对指定窗口进行截图。

## 核心功能

*   **进程窗口查询**：通过进程名称（如 `notepad` 或 `notepad.exe`）搜索并返回该进程拥有的所有窗口详细信息。
*   **窗口状态获取**：提供窗口的句柄（HWND）、进程ID（PID）、标题、类名、可见性、最小化状态以及窗口在屏幕上的坐标矩形。
*   **高性能窗口截图**：利用 Windows.Graphics.Capture API 和 Direct3D 11 进行无感知的窗口截图，甚至可以截取被其他窗口遮挡的后台窗口内容。
*   **图像编码返回**：自动将截取的画面转换为 PNG 格式，并以 Base64 编码形式作为标准的 MCP Image Content 返回给客户端。

## 系统要求

*   **操作系统**：Windows 10 或 Windows 11（需要系统支持 `Windows.Graphics.Capture` API）。
*   **开发环境**：Rust 工具链（基于 2024 Edition，建议使用最新稳定版 Rust）。

## 编译与安装

1. 克隆或下载本项目的源代码。
2. 在项目根目录（`Cargo.toml` 所在目录）打开命令行终端。
3. 执行以下命令进行编译：

```bash
cargo build --release
```

编译完成后，可执行文件将生成在 `target/release/WinCapture.exe`。请记录此文件的绝对路径，以便在 MCP 客户端中配置。

## MCP 客户端配置

要让 MCP 客户端（例如 Claude Code）使用此工具，需要在客户端的配置文件中添加该服务器。

以 Claude Code 为例，运行命令：

```
claude mcp add WinCapture -- X:/example/path/WinCapture.exe
```

配置完成后，重启 MCP 客户端即可生效。

## 提供的 MCP 工具列表

该服务器向 MCP 客户端暴露了以下两个工具（Tools）：

### 1. list_hwnds

*   **描述**：输入进程名，返回匹配窗口的 HWND 列表及窗口元信息。
*   **参数**：
    *   `process_name` (字符串): 目标进程的名称。支持不区分大小写，且带或不带 `.exe` 后缀均可匹配（例如 `"explorer"` 或 `"chrome.exe"`）。
*   **返回值**：结构化的 JSON 数据，包含查找到的所有窗口信息。

### 2. capture_hwnd

*   **描述**：输入 HWND，返回该窗口的 PNG 截图。
*   **参数**：
    *   `hwnd` (字符串): 目标窗口的句柄。支持十六进制（如 `"0x00000000000A1B2C"` 或 `"0x2A"`）或十进制格式。
*   **返回值**：一张 PNG 格式的图片，以 `image/png` 类型的 MCP Content 返回给客户端，AI 客户端可以直接识别和显示该图片。