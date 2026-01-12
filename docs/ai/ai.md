## Project Goal
The objective of this monorepo is to build variants of a p2p superapp for multiple platforms. Apps leverage common rust crates and javascript packages.

## Project Status
*   **Stage**: Exploratory / Unstable. Architecture and APIs are subject to change.

## Tech Stack
*   **Languages**: Rust, TypeScript
*   **Frameworks**: Tauri (Desktop/Mobile), SolidJS (UI)
*   **Package Manager**: pnpm
*   **Build Tools**: Cargo (Rust), Vite (Web/Tauri)

## Core Functionality
*   **P2P app/superapp**: Users run p2p app on their laptop/desktop/mobile. This is an app/superapp node.
*   **Autonomous Identity**: Each p2p node has a unique identity mainly public-private key pair.
*   **Mini-apps** Mini-apps implement custom business logic and hosted in P2P app nodes. UI is generally html-css-js and backend is wasm code.
*   **Mini-App hosting** Mini-app frontend and backend service components can be dynamically added to p2p app node while it is running. 
*   **Mini-App working** Mini-app components are sandboxed wasm/html-css-js components running in a webview or wasm runtime within the node. Wasm/javascript business logic can access filesystem/db/network access using native interfaces exposed to mini-app code. 
*   **Mini-App UI** P2P app node can have a webview to display Mini-app UIs fetched from other app nodes. CLI version of the app does not have webview to display any UI, so CLIs can only host mini-apps.
*   **Encrypted communication** All communication between mini-app as well as base superapp nodes is end-to-end encrypted. as well as Mini-app communication is end-to-end encrypted. 

## Architecture
*   **Repo Organization**: Modular monorepo.
    *   Core logic and P2P networking reside in shared Rust crates (`lib-rust`).
    *   Supports multiple targets: CLI (`app-cmdline`), Cross-platform Tauri app (`app-xplatform`) and browser app (`app-web`)
    *   Tauri app allows dynamic hosting of mini-app components under its wasm runtime. Also allows viewing mini-app UIs within its own webview.
    *   The CLI has no UI, so only has the hosting functionality. 
    *   The web app only has UI, only allows running mini-app frontends in the browser, communicating with backends on cli or tauri app nodes. 
*   **App Layers**:
    *   **Communication layer**: Base communication protocol/interface(s) across superapp peer nodes. Could use one or more of iroh, webrtc or more like NATS, later.
    *   **Mini-app Transport layer**: The Mini-apps could use http-REST, gRPC or others like MQTT as the transport mechanism. The messages sent by mini-app components over the communication layer have information about the transport protocol used. Adapters within the communication-transport boundary ensure correct conversion across the layers.
    *   **RPC mini-app layer**: The business logic or `services` within the mini-app will expose a generic service interface like RPC. RPC interfaces like sendMessage() or response streaming is used to provide pub/sub semantics. Adapters convert to/from transport layer and RPC e.g. HTTP get user/id to RPC getUser(id), or RPC call response stream to HTTP response body
*   **Tauri superapp and app-xplatform IPC Model**:
    *   The `app-xplatform` uses Tauri Commands and Events to bridge the SolidJS superapp UI and the Rust backend.
    *   Superapp UI as well as mini-app UIs run in the Tauri webview.
*   **UI State Management**:
    *   SolidJS Signals/Stores for UI state within the Tauri app
*   **Mini-app structure**:
    *   Business logic is in corresponding mini-app/plugin, WASM backends and Javascript frontends.
    *   Business logic side effects like local file/DB storage or network calls through host functions exposed to the wasm/webview.
    *   mini-app Services are exposed as `wRPC` interfaces with a WASM component model.
    *   mini-apps UI and backend wasm services are bundled and made available as git repos or OCI app images or more formats, that superapp nodes can import.
    *   mini-app services split over multiple nodes as well as sharded horizontally
    *   Apps are versioned
*   **Decentralized P2P compute infra in P2P app/superapp mode**:
    *   Act as Super DHT based registry for nodes and mini-app service discovery, 
    *   Act as a Signalling node to help bootstrap other nodes
    *   Act as a relay node to help app nodes behind NATs
*   **System management and health**:
    *   Observability
    *   Metering of work done (mini-app as well as superapp contributing to P2P infra)
    *   Test automation, unit and end-to-end
    *   **CI/CD**: "Near-dumb" GitHub Workflows. Logic is encapsulated in portable scripts/tools runnable locally; CI merely executes them.
    *   Documentation

## Coding Standards
*   **Rust**: Follow idiomatic Rust patterns. Ensure code is memory-safe and efficient. Use `clippy` for linting and `rustfmt` for formatting.
*   **TypeScript**: Use strict typing. Prefer functional components and signals in SolidJS. Use `eslint` for linting.
*   **General**: Prioritize code reusability in `lib-rust` and `lib-js` for sharing between apps.

## Key modules and Structure
*   `/lib-rust`: Shared Rust crates.
*   `/lib-js`: Shared JavaScript/TypeScript packages.
*   `/docs`: Project documentation.
*   `/app-xplatform`: Tauri-based cross-platform superapp (Mobile/Desktop). Acts as a P2P peer node. UI in SolidJS.
*   `/app-cmdline`: CLI equivalent of the superapp (Rust, no UI).
*   `/app-web`: Web app equivalent (Browser/PWA).
