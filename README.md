# ProcHollowing_Rust

![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange?style=for-the-badge&logo=rust)
![Windows](https://img.shields.io/badge/Windows-10%2B-blue?style=for-the-badge&logo=windows)
![License](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)
![Build](https://img.shields.io/badge/Build-Passing-brightgreen?style=for-the-badge)

A robust Process Hollowing implementation written in Rust for Windows x64 systems.

---

## Project Overview

**ProcHollowing_Rust** is a sophisticated Process Hollowing loader written in Rust for Windows x64 systems. This tool demonstrates advanced PE injection techniques with comprehensive Evasion capabilities, supporting both 32-bit and 64-bit architectures.

### What is Process Hollowing?

Process Hollowing is a technique where a malicious payload is injected into a legitimate process by:

1. Creating a suspended process
2. Unmapping its original executable
3. Writing a new PE image into its memory
4. Resuming the process to execute the payload

### Features

| Feature | Description |
|---------|-------------|
| Cross-Architecture | Supports injection into both x86 and x64 target processes |
| ASLR Support | Handles PE images with and without relocation tables |
| Auto Allocation | Automatically allocates at preferred base address if possible |
| System Process Compatible | Works with Windows system processes (explorer.exe, notepad.exe) |
| Robust Error Handling | Comprehensive logging and error management |
| Evasion Techniques | Anti-debugging, sleep obfuscation, direct syscalls |
| Raw Shellcode Support | Executes raw shellcode via CreateRemoteThread |

### Evasion Techniques

- Sleep Obfuscation - Junk code + random delays
- Anti-Debugging - IsDebuggerPresent check
- PPID Spoofing - Spawns as child of explorer.exe
- Direct Syscalls - NtUnmapViewOfSection bypasses hooks
- EDR Detection - Anti-hook detection in critical DLLs
- Delayed Execution - Timing attacks to evade sandboxes

---

## Getting Started

### Important Note

> This is a x64 executable. You cannot compile this project in x86. This loader is designed to inject into both x86 and x64 processes. You can easily create an x86 Process Hollowing program based on this repository.

### Prerequisites

- Rust 1.70 or higher (Install Rust)
- Windows 10/11 (x64) for execution
- MinGW (for cross-compilation from Linux)

---

## Build

### Build from Source

```bash
git clone https://github.com/0xNyxx/ProcHollowing_Rust.git
cd ProcHollowing_Rust
cargo build --release
