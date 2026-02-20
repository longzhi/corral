# Corral — Agent Skill 沙箱设计

> 为 Agent Skill 脚本提供跨平台的受控执行环境。隔离是手段，能力控制才是目的。

## 项目定位

Agent 执行 Skill 脚本时，当前方案是完全不受限制地在宿主机上运行，存在安全风险（恶意 Skill 可执行恶意脚本）。Corral 提供一个轻量沙箱，让脚本在受控环境中执行，同时通过能力代理（Broker）提供对系统服务的可控访问。

### 核心需求

1. **访问权限** — 只能访问特定目录
2. **网络权限** — 控制是否能访问网络
3. **系统服务调用** — 可控地调用日历、提醒事项、浏览器等系统服务
4. **脚本轻微适配** — 脚本通过 `sandbox-call` SDK 与系统交互，改动极小
5. **跨平台** — macOS、Linux、Windows 三平台支持

---

## 架构概览

```
┌─────────────────────────────────────────────────┐
│                  Agent Runtime                   │
│                                                  │
│  ┌─────────┐    ┌──────────────────────────────┐ │
│  │  Skill  │    │        Sandbox Broker         │ │
│  │ Script  │◄──►│  (运行在宿主机，有完整权限)     │ │
│  │         │    │                               │ │
│  │ 受限环境 │    │  ┌─────────┐ ┌────────────┐  │ │
│  │         │    │  │FS Proxy │ │ Service    │  │ │
│  │ - 文件: ✗│    │  │ 文件读写  │ │ Proxy     │  │ │
│  │ - 网络: ✗│    │  │ 按策略放行│ │ 日历/提醒/ │  │ │
│  │ - 系统: ✗│    │  │         │ │ 浏览器/... │  │ │
│  │         │    │  └─────────┘ └────────────┘  │ │
│  └────┬────┘    └──────────────┬───────────────┘ │
│       │                        │                  │
│       └──── Unix Socket/Pipe ──┘                  │
└─────────────────────────────────────────────────┘
```

**核心思路：脚本跑在受限环境里，所有系统交互都过 Broker 这一道门，策略决定开哪些门。**

---

## 技术栈

| 组件 | 技术 | 说明 |
|------|------|------|
| CLI / Runner / Broker / Policy Engine / Watchdog | **Rust** | 跨平台编译，async 并发，内存安全 |
| libc 拦截库 (libsandbox) | **C** | DYLD interpose / LD_PRELOAD / Detours，~500 行 |
| macOS 系统服务 Helper | **Swift** | 访问 EventKit 等框架 |
| Skill SDK | **多语言** | sandbox-call CLI (Rust) + Python/Node 薄封装 |

### 分工原则

- **C 当门卫** — 只负责拦截 libc 调用（open/connect/execve 等），越薄越好
- **Rust 当大脑** — 所有决策、协调、服务代理、审计

---

## 跨平台隔离方案

| 平台 | 隔离机制 | 特点 |
|------|----------|------|
| **Linux** | bubblewrap (bwrap) | namespace 级隔离，成熟，不需要 root，启动 ~几ms |
| **macOS** | DYLD_INSERT_LIBRARIES + libsandbox.dylib | 用户态 libc 拦截，拦截文件/网络/进程调用 |
| **Windows** | Restricted Token + Job Objects + Detours DLL | Job Objects 管资源限制，Detours 做 API hook |

### C 拦截库 — libsandbox

拦截脚本进程的 libc/Win32 调用：

```c
// macOS: DYLD interpose
#define DYLD_INTERPOSE(_replacement, _original) \
  __attribute__((used, section("__DATA,__interpose"))) \
  static struct { void* r; void* o; } _##_original##_interpose = \
  { (void*)_replacement, (void*)_original };

int my_open(const char *path, int flags, ...) {
    if (!policy_check_path(path, flags)) {
        errno = EACCES;
        return -1;
    }
    return original_open(path, flags, ...);
}
DYLD_INTERPOSE(my_open, open)
```

拦截的函数清单：

| 类别 | 函数 |
|------|------|
| 文件 | `open`, `openat`, `access`, `stat`, `unlink`, `rename` |
| 网络 | `connect`, `bind`, `getaddrinfo` |
| 进程 | `execve`, `posix_spawn` |
| 动态库 | `dlopen` |

---

## Skill Manifest 权限声明

```yaml
# skill.yaml
name: smart-shopping
version: 1.0.0
description: Manage shopping lists with calendar reminders
author: community/alice
entry: ./run.sh
runtime: bash  # bash | python | node

permissions:
  fs:
    read:
      - $SKILL_DIR/**
      - $DATA_DIR/config.json
    write:
      - $WORK_DIR/**
      - $DATA_DIR/**

  network:
    allow:
      - api.example.com:443
      - cdn.example.com:443

  services:
    reminders:
      access: readwrite
      scope:
        lists: ["Shopping"]
    calendar:
      access: read
      scope:
        calendars: ["*"]
        range: 7d
    browser:
      access: open
      scope:
        domains: ["example.com", "*.example.com"]
    notifications:
      access: send

  exec:
    - curl
    - jq
    - python3

  env:
    - LANG
    - TZ
    - SKILL_API_KEY
```

### 预定义变量

| 变量 | 含义 | 生命周期 |
|------|------|----------|
| `$SKILL_DIR` | Skill 安装目录 | 只读，持久 |
| `$WORK_DIR` | 临时工作目录 | 每次运行新建，运行后清理 |
| `$DATA_DIR` | Skill 持久数据 | 可写，跨运行保留 |
| `$SHARED_DIR` | 跨 Skill 共享目录 | 按策略控制 |

### 安装时用户看到的

```
📦 Installing "smart-shopping" v1.0.0 by community/alice

Permissions requested:
  📁 File Access — Read: skill files, config / Write: working dir, skill data
  🌐 Network — api.example.com:443, cdn.example.com:443
  📋 Reminders — Read & Write list: "Shopping"
  📅 Calendar — Read only (next 7 days)
  🌍 Browser — Open URLs on: example.com
  🔔 Notifications — Send notifications

[Allow] [Deny] [Review manifest]
```

---

## Broker API 设计

### 通信协议

- **协议**：JSON-RPC 2.0 over JSON Lines
- **传输**：Unix Socket (macOS/Linux) / Named Pipe (Windows)

### 请求示例

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "reminders.add",
  "params": {
    "list": "Shopping",
    "title": "Buy milk",
    "dueDate": "2025-02-10T18:00:00+08:00"
  }
}
```

### 完整 API 清单

```
Namespace        Method                   参数
─────────────────────────────────────────────────────────
fs               fs.read                  {path}
                 fs.write                 {path, content, encoding?}
                 fs.list                  {path, glob?}
                 fs.delete                {path}
                 fs.stat                  {path}

network          network.http             {method, url, headers?, body?}
                 network.download         {url, saveTo}

reminders        reminders.list           {list?, completed?}
                 reminders.add            {list, title, dueDate?, notes?}
                 reminders.update         {id, title?, dueDate?, notes?}
                 reminders.complete       {id}
                 reminders.delete         {id}

calendar         calendar.list            {calendar?, from?, to?}
                 calendar.get             {id}
                 calendar.create          {calendar, title, start, end, ...}
                 calendar.update          {id, ...}
                 calendar.delete          {id}

browser          browser.open             {url}

clipboard        clipboard.read           {}
                 clipboard.write          {text}

notifications    notifications.send       {title, body, sound?}

exec             exec.run                 {command, args[], cwd?, timeout?}

env              env.get                  {name}
```

### 错误码

```
标准 JSON-RPC:
  -32600  Invalid request
  -32601  Method not found
  -32602  Invalid params
  -32603  Internal error

Corral 特有:
  -32001  Permission denied (服务未授权)
  -32002  Scope violation (超出 scope 范围)
  -32003  Rate limited
  -32004  Timeout
  -32005  Service unavailable (平台不支持此服务)
  -32006  Network denied (域名不在白名单)
  -32007  Path denied (路径不在允许范围)
```

---

## Skill SDK

脚本通过 `sandbox-call` CLI 或语言 SDK 与 Broker 通信：

### Bash

```bash
events=$(sandbox-call calendar.list --from today --to "+7d")
sandbox-call reminders.add --list Shopping --title "Buy milk" --due "2025-02-10T18:00:00"
result=$(sandbox-call network.http --method GET --url "https://api.example.com/data")
sandbox-call notifications.send --title "Done!" --body "Shopping list updated"
```

### Python

```python
from sandbox import api
events = api.calendar.list(from_date="today", to="+7d")
api.reminders.add(list="Shopping", title="Buy milk")
```

### Node

```javascript
import { sandbox } from '@sandbox/sdk'
const events = await sandbox.calendar.list({ from: 'today', to: '+7d' })
await sandbox.reminders.add({ list: 'Shopping', title: 'Buy milk' })
```

`sandbox-call` 本身是 Rust 编译的 ~1MB 静态二进制，放在沙箱内 `$PATH` 里。Python/Node SDK 是薄封装，底层都走同一个 socket。

---

## Service Adapter 跨平台实现

### 平台实现矩阵

| 服务 | macOS | Linux | Windows |
|------|-------|-------|---------|
| Calendar | EventKit (Swift helper) | D-Bus → GNOME Calendar / ical 文件 | COM → Outlook |
| Reminders | EventKit (Swift helper) | D-Bus → GNOME To Do / todo.txt | COM → Outlook Tasks |
| Browser | NSWorkspace.open() | xdg-open | ShellExecute |
| Notifications | UNUserNotification | notify-send / D-Bus | Toast Notification API |
| Clipboard | NSPasteboard | xclip / xsel / wl-copy | Win32 Clipboard API |

### macOS Swift Helper 架构

Rust Broker 不直接链接 Swift/ObjC，通过独立的 Swift helper CLI 进程间通信：

```
Broker (Rust) ── stdin/stdout JSON ──► calendar-helper (Swift) ──► EventKit
```

### Adapter 注册

运行时根据平台自动选择实现，如果系统没有对应服务则返回 `-32005 Service unavailable`。Linux 侧会检测桌面环境（GNOME/KDE）选择对应实现，无桌面环境时 fallback 到文件型方案（ical/todo.txt）。

---

## 沙箱进程生命周期

```
Phase 1: PREPARE
  ├─ 解析 skill.yaml manifest
  ├─ 权限预检（是否已批准）
  └─ 创建 $WORK_DIR (临时) / 确保 $DATA_DIR (持久)

Phase 2: SETUP SANDBOX
  ├─ 平台特定隔离:
  │   [Linux]   构建 bwrap 命令 (namespace/挂载/网络隔离)
  │   [macOS]   设置 DYLD_INSERT_LIBRARIES + 受限 ENV
  │   [Windows] CreateRestrictedToken + Job Object
  ├─ 启动 Broker (监听 Unix Socket / Named Pipe)
  └─ 注入 SDK (sandbox-call 放入 $PATH)

Phase 3: EXECUTE
  ├─ 启动脚本进程 (独立进程组)
  ├─ 启动 Watchdog (超时/内存/频率监控)
  └─ 运行期:
       脚本 ──sandbox-call──► Broker ──Policy✓──► Adapter ──► 系统服务
       脚本 ──直接 open()──► libsandbox 拦截 ──► EACCES

Phase 4: CLEANUP
  ├─ 收集 stdout/exit code → 返回给 Agent
  ├─ rm -rf $WORK_DIR
  ├─ 关闭 Broker socket
  ├─ 确认进程组全部退出
  └─ 生成运行报告 (审计日志)
```

### 审计日志

所有 Broker 调用天然有完整日志：

```json
{
  "skill": "smart-shopping",
  "run_id": "run-abc123",
  "duration_ms": 2340,
  "exit_code": 0,
  "broker_calls": [
    {"method": "reminders.add", "allowed": true, "ms": 45},
    {"method": "network.http", "allowed": true, "ms": 230}
  ],
  "blocked_calls": [
    {"method": "fs.read", "path": "/etc/passwd", "reason": "path denied"}
  ],
  "resources": {
    "peak_memory_mb": 34,
    "cpu_time_ms": 890
  }
}
```

---

## Broker 内部架构

```
┌─────────────────────────────────────────┐
│              Sandbox Broker              │
│                                         │
│  ┌──────────┐   ┌────────────────────┐  │
│  │ Socket   │   │  Policy Engine     │  │
│  │ Server   │──►│  manifest.yaml     │  │
│  │          │   │  → 权限检查        │  │
│  └──────────┘   │  → scope 验证       │  │
│                 │  → rate limiting    │  │
│                 └────────┬───────────┘  │
│                          │              │
│              ┌───────────▼───────────┐  │
│              │   Service Adapters    │  │
│              │                       │  │
│              │  ┌─────┐┌─────┐┌───┐ │  │
│              │  │macOS││Linux││Win│ │  │
│              │  └─────┘└─────┘└───┘ │  │
│              └───────────────────────┘  │
└─────────────────────────────────────────┘
```

请求流程：`脚本 → Socket → Policy Engine 检查 → Service Adapter 执行 → 返回结果`

---

## 安全边界

### 能防住的

- Skill 脚本读写不该碰的文件
- 脚本偷偷联网
- 脚本启动不该启动的进程
- 资源滥用（fork bomb、吃光内存）
- 未授权的系统服务调用

### 防不住的

- 恶意编译的 native binary 直接用 syscall 绕过 libc 拦截（macOS/Windows）
- Linux 侧 bwrap 是内核级隔离，不存在此问题

**威胁模型**：不受信任的社区脚本可能行为不当，不是对抗 APT。脚本都是解释器运行，走 libc，用户态拦截有效。

---

## 项目结构

```
corral/
├── Cargo.toml
├── src/
│   ├── main.rs                # CLI 入口
│   ├── manifest.rs            # skill.yaml 解析
│   ├── policy.rs              # 权限策略引擎
│   ├── broker/
│   │   ├── mod.rs             # Broker 主循环
│   │   ├── jsonrpc.rs         # JSON-RPC 协议
│   │   └── router.rs         # method → adapter 路由
│   ├── adapters/
│   │   ├── mod.rs             # trait 定义
│   │   ├── calendar/          # macOS / Linux / Windows
│   │   ├── reminders/         # macOS / Linux / Windows
│   │   ├── browser.rs
│   │   ├── notifications.rs
│   │   ├── clipboard.rs
│   │   ├── filesystem.rs
│   │   └── network.rs
│   ├── platform/
│   │   ├── macos.rs           # DYLD 启动逻辑
│   │   ├── linux.rs           # bwrap 启动逻辑
│   │   └── windows.rs        # Restricted Token + Job
│   ├── watchdog.rs            # 资源监控 + 超时
│   └── audit.rs               # 审计日志
├── libsandbox/                # C 拦截库 (~500行)
│   ├── interpose_macos.c
│   ├── interpose_linux.c
│   ├── hook_windows.c
│   ├── policy.c
│   └── comm.c
├── sdk/
│   ├── sandbox-call/          # CLI SDK (Rust)
│   ├── python/                # pip install sandbox-sdk
│   └── node/                  # npm install @sandbox/sdk
└── helpers/
    ├── calendar-helper-macos/ # Swift CLI
    └── reminders-helper-macos/
```

---

## 开发计划

| 阶段 | 内容 | 周期 |
|------|------|------|
| Phase 1 | Linux bwrap + fs/network 隔离 + sandbox-call CLI | 2 周 |
| Phase 2 | macOS DYLD + Broker + fs/network adapters | 2 周 |
| Phase 3 | 系统服务 adapters (calendar/reminders/browser) | 2 周 |
| Phase 4 | Windows 支持 + 完善 | 2 周 |

**预估总代码量：~6000-8000 行**（Rust + C + Swift + SDK）

---

## 设计原则

1. **声明式权限** — Skill 声明"我需要什么"，不是"我要怎么做"
2. **最小权限** — 不声明的默认 deny
3. **能力代理** — 不做纯隔离，通过 Broker 提供受控的系统服务访问
4. **跨平台统一 API** — 脚本写一次 `sandbox-call calendar.create`，三平台通用
5. **审计免费获得** — 所有调用过 Broker 的操作天然有完整日志
