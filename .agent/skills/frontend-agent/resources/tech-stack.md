# Frontend Agent - Tech Stack Reference

## Core Stack
- **Framework**: Next.js 16+ (App Router)
- **Library**: React 19+
- **Language**: TypeScript 5.0+
- **Desktop**: Tauri 2.0+

## UI & Styling
- **Styling**: Tailwind CSS v4
- **Components**: shadcn/ui (Radix UI based)
- **Icons**: Lucide React
- **Animation**: Framer Motion (optional)

## State Management
- **Server/IPC State**: TanStack Query v5 (React Query)
- **Client State**: Zustand (Global), React Context (Scoped)
- **URL State**: Nuqs (Type-safe search params)

## Data Fetching / IPC
- **Tauri**: `@tauri-apps/api`, `@tauri-apps/plugin-*`
- **HTTP**: Axios or Fetch (for external APIs not via Rust)

## Testing
- **Unit**: Vitest + React Testing Library
- **E2E**: Playwright (if applicable)
