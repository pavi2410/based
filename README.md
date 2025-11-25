# Based - The Everything Database App

A free, open-source database explorer and management tool for developers.

## Features
- Supports SQLite and MongoDB (PostgreSQL planned)
- Free & Open Source
- Privacy-focused - No backend, no data sent to servers

## Project Structure

```
based/
├── apps/
│   └── desktop/        # Main Tauri desktop application
├── packages/           # Shared packages (future)
├── crates/             # Shared Rust crates (future)
└── mise.toml           # Task runner configuration
```

## Quick Start

### Prerequisites
- Node.js 20+
- Bun (latest)
- Rust (latest stable)
- Tauri prerequisites for your OS

### Installation

```bash
bun install
# or
mise run install
```

### Development

```bash
# Start development server (frontend only)
bun dev
# or
mise run dev

# Start Tauri development mode (full app)
bun tauri dev
# or
mise run tauri:dev
```

### Building

```bash
# Build frontend
bun run build

# Build Tauri app
bun tauri build
# or
mise run tauri:build
```

## Available Tasks (via mise)

- `mise run dev` - Start frontend dev server
- `mise run tauri:dev` - Start Tauri development mode
- `mise run build` - Build for production
- `mise run tauri:build` - Build Tauri app
- `mise run install` - Install dependencies
- `mise run clean` - Clean build artifacts
- `mise run lint` - Lint code
- `mise run format` - Format code
- `mise run type-check` - Type check TypeScript

## Documentation

See [apps/desktop/README.md](./apps/desktop/README.md) for desktop app details.
