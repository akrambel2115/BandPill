# 🌐 BandPill

[![CI](https://github.com/akrambel2115/BandPill/actions/workflows/ci.yml/badge.svg)](https://github.com/akrambel2115/BandPill/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2011%20%7C%2010-0078d4.svg)](#)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Binary Size](https://img.shields.io/badge/Binary%20Size-~580%20KB-success.svg)](#)

A high-performance, lightweight network bandwidth tracker and **Data Saver & Anti-Leak Shield** built specifically for Windows 11 in **100% pure Rust**.

**BandPill** docks directly onto your taskbar right beside the Wi-Fi, battery, and clock icons to display today's internet consumption in real time (`230 MB` or `1.4 GB`), and features a built-in one-click **Data Saver & Anti-Leak Shield** to prevent Windows from secretly wasting your bandwidth on background P2P update uploads, telemetry, and hidden transfer services.

---

## ⚡ Performance & Architecture

BandPill is engineered from the ground up in native Rust utilizing direct Win32, DWM, and IP Helper APIs:

| Metric | BandPill Specification | Advantage |
| :--- | :--- | :--- |
| **Standalone Binary Size** | **~580 KB** | Ultra-lightweight, zero bloat |
| **Startup Time** | **< 15 ms** | Instantaneous launch |
| **RAM Footprint** | **~6 - 9 MB** | Negligible system resource impact |
| **External Dependencies** | **0 (Zero)** | Pure native Win32 executable |
| **UI Rendering** | **Direct DWM & GDI** | Fluid 60 FPS, High-DPI crisp typography |

---

## ✨ Features

### 1. Dual Display Modes
- **Taskbar Pill Widget**: A sleek, horizontal rounded badge (`230 MB`) docked directly onto your taskbar beside the system tray.
  - Fully draggable with persistent coordinate saving across reboots.
  - Position lock toggle (`✔ Position Locked`) to prevent accidental movement.
  - Windows 11 dark/light theme aware with subtle hover glow.
- **Dynamic System Tray Icon**: A high-DPI notification area icon rendered in native **Segoe UI Variable Display** typography that dynamically reflects today's usage in real time.

### 2. Accurate Daily Tracking & Filtering
- **Automatic Midnight Rollover**: Seamlessly resets counters at `00:00:00` and archives previous days into a rolling 30-day historical log.
- **Hardware Isolation**: Filters out virtual adapters (`WSL`, `Docker`, `Hyper-V`, `VMware`, `Tailscale`, `WireGuard`, and `Loopback`) in physical tracking mode, isolating your actual Wi-Fi, Ethernet, or Cellular consumption.
- **Reboot Persistence**: Saves counters to `%LOCALAPPDATA%\BandPill\` every 10 seconds to survive sudden power cuts or restarts.

### 3. Expandable Applications Bandwidth Monitor
- **Live Transfer Speeds & Bars**: Displays all active processes consuming data with live transfer speed (`⚡ 1.4 MB/s` or `Idle`), whole-day consumption total, and visual proportional usage bars.
- **Expandable List**: Switch dynamically between **`Top 5`**, **`Top 10`**, and **`All`** active applications.
- **Persistent Stop & Block**: Click **`🛑 Block`** on any application to immediately terminate its running processes **and** continuously auto-kill background respawn attempts.
- **OS Protection**: Essential Windows processes (`dwm.exe`, `csrss.exe`, `services.exe`, `lsass.exe`) are guarded with a `🛡️ System` badge to prevent accidental termination.

### 4. 🚨 High-Bandwidth Download Alerts
- Automatically monitors rolling transfer windows (e.g. 100 MB downloaded within 60 seconds).
- Pops up a Windows 11 Fluent topmost toast with a gentle exclamation chime:
  - Displays Process Name, PID, executable, downloaded MB, duration, and live transfer rate.
  - Quick action buttons: **🛑 Kill Process** or **✔ Let Continue** (snoozes alerts for 15 minutes).
  - One-click trigger threshold configuration (**`50 MB`**, **`100 MB`**, **`250 MB`**, **`500 MB`**).

---

## 🛡️ Windows Background Leak Prevention (Data Saver)

Windows 11 runs heavy background tasks that secretly consume bandwidth and upload data. You can control and stop all of them with **one click**:

### 1. Stop Windows from Secretly Uploading Updates (Delivery Optimization)
- **The Issue**: Windows Delivery Optimization turns your PC into a peer-to-peer (P2P) server, uploading Windows update files to random computers across the internet.
- **The One-Click Fix**: Click **🛑 Stop Uploads**. This sets `DODownloadMode=0` (HTTP only / no peering) and stops the `DoSvc` background service.

### 2. Set Your Network to "Metered Connection"
- **The Issue**: On normal connections, Windows downloads large OS updates, driver packages, and telemetry silently.
- **The One-Click Fix**: Click **📶 Set as Metered**. Halts automatic background OS downloads and telemetry syncs.

### 3. Kill the Background Traffic Service (BITS)
- **The Issue**: BITS continuous background downloads consume hidden bandwidth.
- **The One-Click Fix**: Click **🛑 Kill & Manual**. Immediately terminates active background downloads and changes BITS startup type to Manual.

### ⚡ Master One-Click Guard
Click **`⚡ Apply All 3 Protections Now`** to execute all 3 mitigations simultaneously in a single click!

---

## 🚀 Installation & Running

### Portable Executable (Recommended)
1. Download **`BandPill.exe`** from the latest [GitHub Release](https://github.com/akrambel2115/BandPill/releases).
2. Double-click to run. No installer or runtime prerequisites required!

### Building from Source

```powershell
# Prerequisites: Rust 1.75+ and Visual Studio C++ Build Tools
git clone https://github.com/akrambel2115/BandPill.git
cd BandPill

# Build optimized release binary
cargo build --release

# The compiled standalone executable is at:
# target/release/BandPill.exe
```

---

## 🧪 Testing

The codebase includes automated unit and integration tests covering configuration persistence, adapter classification, traffic calculations, app blocking lifecycle, and formatting:

```powershell
# Run all tests
cargo test
```

---

## 📁 Repository Structure

```
bandpill/
├── Cargo.toml          # Package manifest and dependencies
├── build.rs            # Windows resource compilation (embeds app.ico and manifest)
├── app.ico             # Application icon
├── src/
│   ├── main.rs         # WinMain entry point, message pump, single-instance IPC check
│   ├── lib.rs          # Library root exposing core modules
│   ├── config.rs       # Configuration serde, autostart Windows registry integration
│   ├── storage.rs      # Day state persistence, midnight rollover, 30-day archive
│   ├── tracker.rs      # Network tracking engine, active adapter detection, process I/O
│   ├── datasaver.rs    # Data Saver & Anti-Leak Shield (Delivery Optimization, Metered, BITS)
│   ├── ipc.rs          # Localhost TCP single-instance lock and messaging
│   ├── net/
│   │   ├── adapter.rs  # Windows IP Helper API adapter queries, virtual filtering
│   │   └── process.rs  # Network connection tables, process I/O counters, friendly names
│   ├── ui/
│   │   ├── theme.rs    # Windows 11 dark/light theme detection and DWM rounded styling
│   │   ├── icon.rs     # High-DPI GDI dynamic tray icon generator
│   │   ├── tray.rs     # Notification area icon manager and right-click menus
│   │   ├── pill.rs     # Floating draggable rounded taskbar pill widget
│   │   ├── flyout.rs   # Windows 11 Fluent flyout dashboard (Overview & Data Saver tabs)
│   │   └── toast.rs    # Dynamic high-bandwidth download alert toast
│   └── utils.rs        # String conversions, formatting helpers, UAC elevation
├── tests/              # Automated unit and integration test suite
│   ├── adapter_tests.rs
│   ├── config_tests.rs
│   ├── ipc_tests.rs
│   ├── storage_tests.rs
│   ├── tracker_tests.rs
│   └── utils_tests.rs
├── .github/workflows/
│   ├── ci.yml          # GitHub Actions CI for build and tests
│   └── release.yml     # Automated release packaging on tag push
├── CONTRIBUTING.md     # Contribution guidelines and development setup
└── LICENSE             # MIT License
```

---

## 🤝 Contributing

Contributions, bug reports, and suggestions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on code style, testing requirements, and the pull request process.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
