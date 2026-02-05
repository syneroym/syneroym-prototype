# Syneroym

> ⚠️ **Status: Exploratory / Unstable**
> 
> This project is **WORK IN PROGRESS** and is under active exploration and development. The architecture, APIs, data models, and overall direction are subject to frequent change. Nothing here should be considered stable or production-ready at this stage. The repository is public for transparency and ease of sharing, not as an invitation for general use or contribution at this time.

## Introduction

Syneroym is aiming to be a platform for autonomous cooperative value exchange. It enables users (individuals, small orgs/businesses) to host services (or call them local first apps, mini-apps) on peer nodes running on diverse types of hosts (PCs, mobile, cloud). These apps can be used by consumers (which in turn could be other services too), all participating in a decentralized interaction ecosystem.

## Functionality highlights (WIP)
- Single `Peer` app running on each host, that houses (or enables access to) apps within itself. Peer could be behind the NAT without external IP.
- Developers write mini-apps or services that compile to variety of deployable modules. These miniapps can be dropped into/removed from hosts as needed.
- All mini-apps are sandboxed in their private environment with quota restrictions.
- Consumers use these mini-apps via a GUI, browser or custom code that talks to mini-apps via a local http proxy. 
- All communication from over the network is encrypted. Each host and app/service has a unique `hash` identity.
- Most data passes directly from peer to peer with some very basic shared servers for initial peer discovery. 
  - Data-relays are used as last fallback if NAT or firewall limitations disallow direct communication. But still communication is end-to-end encrypted, so data cannot be seen by relays too.

## Architecture highlights (WIP)
- Peer discovery signalling using iroh and webrtc, with UDP hole punching. TCP relay as fallback.
- Unique hashes as identity, encryption based on public-private keys, self-signed digital certs. For nodes, services.
- [Bittorrent BEP 44](https://www.bittorrent.org/beps/bep_0044.html) as a service registry for discovery
- Rust based peer node managing all tunnelling between users and mini-apps/services deployed behind/into it.
- Business logic typically packaged as WASM modules running in sandboxed WASM runtime (more alternatives later). UI as HTML-CSS-JS files. `Side-Effects` like files/DB/network available via Host functions available to `guest mini-apps` as `providers`. 

## Vision
For the broader, longer term vision, ideation artifacts etc. please refer to the [Foundation](https://github.com/syneroym/foundation) repository.

## This Repo
This monorepo contains different variants of the Syneroym application, like web-mobile app, headless server app, libraries, and supporting tools. Use based on desired nature of use and installation platform capabilities.:
- **Applications**: Cross-platform user interfaces (Web/Mobile/Desktop) and CLI tools.
- **Libraries**: Shared Rust crates and JavaScript/TypeScript packages.
- **Examples**: Demonstration projects and mini-apps.

## Code Organization

The repository is organized into several key directories:

### Applications
- **[`app-cli`](./app-cli/)**: A command-line interface (CLI) for Syneroym, providing headless interaction with the ecosystem, accessible via browser. Currently this is the main app used for core functionality development and testing. 
- **[`app-xplatform`](./app-xplatform/)**: The user-facing application with a UI on top of the above cli functionality (both for node management and miniapp UIs). Curently this is just a shell, but the peer node code will be wired in after the functionality is tested with cli. Uses [Tauri](https://tauri.app/), [SolidJS](https://www.solidjs.com/), and [Tailwind CSS](https://tailwindcss.com/) and rust libs shared with the above CLI. It runs on Desktop (macOS, Windows, Linux) and Mobile (Android, iOS). 

### Key Libraries/crates
- **[`lib-rust`](./lib-rust/)**: A collection of modular Rust crates powering the core logic, networking, storage, and protocols.
  - Overall wiring and tunnelling: `node`
  - P2P Networking with iroh and/or webrtc: `net`, `net-iroh`, `net-webrtc`, `signaling-server`
- **[`lib-js`](./lib-js/)**: Shared JavaScript/TypeScript packages used by the frontend applications. Currently, this is just a placeholder.

### Examples
Example projects to demonstrate that applications with various types of features can be handled p2p also.

## Getting Started

### Dev setup
Install [mise](https://mise.jdx.dev) to manage tool versions. Run `mise install` to prepare your dev environment. A `mise.toml` is provided in the root.

Alternatively, you can install various tools manually. Ensure you have the following installed in your development environment:
- **Rust**: Stable toolchain (managed via `rustup`).
- **Node.js**: LTS version (v22+ recommended).
- **pnpm**: Package manager for Node.js projects.

### Build & Run

1.  **Install Dependencies**:
    ```bash
    pnpm install
    ```

2.  **Run the CLI and test**:
    ```bash
    # Run the sample web app:
    RUST_LOG="miniapp=debug,info" cargo run -p miniapp-demo1-web -- --port 3001
    # Run the app (below command or VSCode debugger)
    cargo run -p app-cli -- run-peer --config-file app-cli/config.toml
    ```
    Open browser visit http://localhost:3001, as well as http://demo3001.localhost:8001/, all functionality should work

3.  **Run the Cross-Platform App (Desktop)**:
    ```bash
    cd app-xplatform
    pnpm tauri dev
    ```

4.  **Run Tests**:
    ```bash
    cargo test --workspace
    ```

## Documentation

- [Developer Guide](docs/developer.md)
