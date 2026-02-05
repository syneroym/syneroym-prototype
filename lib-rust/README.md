# Syneroym Rust Libraries (`lib-rust`)

This directory contains the core Rust crates that power the Syneroym ecosystem. These crates are designed to be modular and reusable across different interfaces (CLI, Desktop, Mobile).

## Crates Overview

### Core & Application Logic
- **`node`**: Core node logic, orchestrating networking, storage, and application management.
- **`common`**: Shared utilities, types, and helper functions used across the workspace.

### Networking & P2P
- **`net`**: High-level networking abstractions.
- **`net-iroh`**: Networking implementation based on [Iroh](https://iroh.computer/).
- **`net-webrtc`**: WebRTC-based networking capabilities.
- **`signaling-server`**: Facilitates connection establishment between peers.

