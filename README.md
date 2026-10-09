# ZapFast (Calls & Extended Edition)

**WhatsApp, native, ultra-fast, with full voice & video call support.**

> [!NOTE]
> This repository is an **enhanced fork** of [crmne/zapfast](https://github.com/crmne/zapfast). While the original ZapFast focused on lightweight text chat, this fork brings full **Voice & Video Calling**, **Call History**, **Starred & Pinned Messages**, **Story Audio**, **Animated Stickers**, and WhatsApp Web-style modern workflow enhancements to the native desktop experience.

Written in pure Rust with [egui](https://github.com/emilk/egui) and [whatsapp-rust](https://github.com/oxidezap/whatsapp-rust). ZapFast links directly to your phone as an official companion device with **no browser engine / no Electron overhead**, launching in under a second and using ~200 MB RAM compared to 1.1+ GB on WhatsApp Web.

<picture>
  <source media="(prefers-color-scheme: light)" srcset="docs/screenshot-light.png">
  <img src="docs/screenshot.png" alt="ZapFast conversation view">
</picture>

---

## 🚀 What's New in this Fork

| Feature | Description |
| :--- | :--- |
| 📞 **Voice & Video Calls** | Native WebRTC calls, 30 FPS hardware camera capture, OpenH264 video acceleration, and low-latency audio via `cpal` + `opus`. |
| 📋 **Call History (Logs)** | Dedicated Calls tab with answered/missed logs, WhatsApp Web-style filter chip, and unread badge indicator. |
| 🌟 **Starred Messages** | Star/unstar messages via right-click menu, app-state sync, and quick filter/drawer to view all starred items. |
| 📌 **Pinned Messages** | Pin messages in conversations with 24h, 7d, or 30d durations; pinned banner displayed atop the chat room. |
| 🎬 **Status & Story Audio** | Fixed AAC audio decoding on MP4 status videos via custom pure-Rust `symphonia` demuxer patch. Delete/revoke own statuses directly. |
| ✨ **Animated Stickers** | Full animated sticker support (Lottie/WebP) with continuous autoplay in focus and sharp previews in picker. |
| 🚫 **Block / Unblock** | Block or unblock contacts straight from chat options or contact profile info. |
| ⏳ **Disappearing Messages** | Manage ephemeral message timers (24 hours, 7 days, 90 days) per chat. |
| 🎨 **Modern WhatsApp Web UI** | Refined sidebar rail navigation, toggle panels, accurate unread badge count, and fixed system tray. |

Plus latest upstream additions: open chats from links/URIs (`whatsapp:`), phone numbers in message text open actions to start a chat, and message info shortcut.

**Want Spotify just as fast and native?** [Spotifast](https://spotifast.rocks)
is ZapFast's sibling. Both are built on
[fastframe](https://github.com/crmne/fastframe).

---

## 💻 Cross-Platform Support

ZapFast runs natively on all major desktop operating systems:

- **Linux** (x86_64, aarch64) — Wayland & X11 with hardware-accelerated rendering.
- **Windows** (x86_64, arm64) — Native Win32 windowing, no WSL required. Portable zip & setup installer.
- **macOS** (Universal: Apple Silicon M1-M4 & Intel) — Native Cocoa/Metal with Retina display and camera/mic permissions.

---

## 📥 Download

Precompiled binaries for Linux, Windows, and macOS are automatically built on every release:

👉 **[Download the Latest Release on GitHub](https://github.com/crmne/zapfast/releases)** *(or from your fork's Releases tab)*

- **Windows**: `zapfast-*-x86_64-pc-windows-msvc.zip` (portable) or `*-setup.exe` (installer)
- **Linux**: `zapfast-*-x86_64-unknown-linux-gnu.tar.gz`
- **macOS**: `zapfast-*-macos-universal.dmg` or `*.zip`

---

## 🛠️ Building from Source

### Prerequisites

- **Rust** 1.85+ (`rustup default stable`)
- **Linux dependencies** (Ubuntu/Debian):
  ```sh
  sudo apt-get install -y libxkbcommon-dev libwayland-dev libgl1-mesa-dev libasound2-dev libssl-dev pkg-config cmake perl
  ```

### Build & Run

```sh
# Clone the repository
git clone https://github.com/crmne/zapfast.git
cd zapfast

# Compile release binary
OPENSSL_NO_VENDOR=1 cargo build --release --bin zapfast

# Run ZapFast
./target/release/zapfast
```

For offline demo mode (inspect UI layouts without logging in):
```sh
cargo run --features demo -- --demo            # offline sample chats, no WhatsApp connection
cargo run --features demo -- --demo-page phone-menu
```

---

## 🔒 Security & Privacy

- **Local Storage**: All chat messages, media caches, and credentials are encrypted on disk with SQLCipher and argon2id.
- **End-to-End Encryption**: Retains WhatsApp's native Signal Protocol E2EE for chats, media, and WebRTC calls.
- **Open Source**: Auditable, telemetry-free, and tracker-free.

---

## 📜 Disclaimer & Credits

- ZapFast is an unofficial client and is not affiliated with WhatsApp or Meta. Using third-party clients carries inherent terms of service risks.
- **Original Project**: Created with love by [Carmine Paolino](https://github.com/crmne) ([crmne/zapfast](https://github.com/crmne/zapfast)).
- **Protocol Library**: Powered by [oxidezap/whatsapp-rust](https://github.com/oxidezap/whatsapp-rust).
- **License**: MIT.
