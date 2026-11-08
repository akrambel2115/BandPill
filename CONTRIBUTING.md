# Contributing to BandPill

Thank you for your interest in contributing to **BandPill**! This project is built in pure Rust using native Win32 APIs for maximum performance, minimal resource usage, and seamless integration with Windows 11 Fluent Design.

---

## 🛠️ Development Setup

### Prerequisites
1. **Windows 10 or Windows 11** (x86_64 or aarch64)
2. **Rust & Cargo** (1.75+ recommended):
   ```powershell
   winget install Rustlang.Rustup
   rustup default stable-x86_64-pc-windows-msvc
   ```
3. **Visual Studio C++ Build Tools** (for MSVC linker)

### Clone & Build
```powershell
git clone https://github.com/akrambel2115/BandPill.git
cd BandPill

# Build in debug mode
cargo build

# Run locally
cargo run --bin BandPill

# Build optimized release binary
cargo build --release
```

---

## 🧪 Testing

We require all contributions to pass automated tests and code checks before merging:

```powershell
# Run all unit and integration tests
cargo test

# Run clippy for linting
cargo clippy --all-targets --all-features

# Format checking
cargo fmt --check
```

---

## 📁 Codebase Architecture

```
src/
├── main.rs         # WinMain entry point, message pump, single-instance IPC check
├── lib.rs          # Library root exposing core modules for testing & embedding
├── config.rs       # Configuration serde, autostart Windows registry integration
├── storage.rs      # Day state persistence, midnight rollover, 30-day archive
├── tracker.rs      # Network tracking engine, active adapter detection, process I/O
├── datasaver.rs    # Data Saver & Anti-Leak Shield (Delivery Optimization, Metered, BITS)
├── ipc.rs          # Localhost TCP single-instance lock and messaging
├── net/
│   ├── adapter.rs  # Windows IP Helper API adapter queries, virtual filtering
│   └── process.rs  # Network connection tables, process I/O counters, friendly names
├── ui/
│   ├── theme.rs    # Windows 11 dark/light theme detection and DWM rounded styling
│   ├── icon.rs     # High-DPI GDI dynamic tray icon generator
│   ├── tray.rs     # Notification area icon manager and right-click menus
│   ├── pill.rs     # Floating draggable rounded taskbar pill widget
│   ├── flyout.rs   # Windows 11 Fluent flyout dashboard (Overview & Data Saver tabs)
│   └── toast.rs    # Dynamic high-bandwidth download alert toast
└── utils.rs        # String conversions, formatting helpers, UAC elevation
```

---

## 📋 Pull Request Guidelines

1. **Create a Feature Branch**:
   ```powershell
   git checkout -b feature/my-new-feature
   ```
2. **Keep Commits Clean & Descriptive**:
   - Use conventional commit messages (`feat: ...`, `fix: ...`, `docs: ...`, `test: ...`).
3. **Add Tests**:
   - For bug fixes, include a regression test.
   - For new features, include unit and integration tests under `tests/`.
4. **Ensure Clean Builds**:
   - Code must compile with zero warnings on `cargo check` and `cargo test`.
5. **Open a Pull Request**:
   - Submit your pull request to the `master` branch with a clear description of the change.

---

## 📄 License
By contributing to this repository, you agree that your contributions will be licensed under the [MIT License](LICENSE).
