# Syneroym Cross-Platform App (`app-xplatform`)

The primary user-facing application for Syneroym, capable of running on Desktop (macOS, Windows, Linux) and Mobile (Android, iOS).

## Tech Stack

- **Framework**: [Tauri v2](https://v2.tauri.app/)
- **Frontend**: [SolidJS](https://www.solidjs.com/)
- **Styling**: [Tailwind CSS](https://tailwindcss.com/)
- **Build Tool**: [Vite](https://vitejs.dev/)

## Development

### Prerequisites

Ensure you have Rust and Node.js installed. See the root [README](../../README.md) for details.

### Running in Development Mode

To start the application in development mode with hot-reloading:

```bash
cd app-xplatform
pnpm install
pnpm tauri dev
```

### Building for Production

To build the application for your current platform:

```bash
pnpm tauri build
```

