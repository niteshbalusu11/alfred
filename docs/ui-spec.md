# Alfred UI Specification (Phase I)

- Last Updated: 2026-02-20
- Scope: iOS front-end design and implementation rules for Phase I
- Audience: iOS engineers and autonomous coding agents

## 1) Source Of Truth

This document is the source of truth for iOS UI/UX implementation in Phase I.

If a front-end issue conflicts with ad hoc styling choices, follow this spec and update the issue notes.

## 2) Product Direction

1. Dark-mode-only experience for Phase I.
2. Monochrome cartoony visual language is the app-wide style baseline.
3. Home is the primary interaction surface.
4. Connectors must be first-class and future-extensible (Google now, others later).
5. Top-tab navigation remains the primary app structure after auth, with swipe paging between tabs.

## 3) Startup + Auth Journey (FE12)

First-launch/auth routing must be deterministic and Clerk-native:

1. App launch enters a bootstrap state while session/auth state is resolved.
2. If unauthenticated, route to startup entry screen and present Clerk `AuthView` as the login surface.
3. If authenticated and bootstrap succeeds, route directly to app shell.
4. If authenticated but bootstrap fails, show recoverable UX with retry and sign-out actions.
5. Post-login lands in Home tab by default unless a supported deep link route override exists.

Do not replace Clerk login UI with custom credential forms in Phase I.

## 4) Navigation Architecture

Primary tabs:

1. Home
2. Activity
3. Connectors
4. Profile/account actions are accessed from the top-right account button.

Architecture rules:

1. Use `TabView` at app root with `.page` style for horizontal swipe navigation and no bottom system tab bar.
2. Use a top tab control (segmented/native-feeling) bound to the same tab selection state.
3. Use one `NavigationStack` per tab (independent history).
4. Route deep links through centralized router logic, not per-screen ad hoc handlers.
5. Keep tab identity centralized in one enum.

## 5) Screen Responsibilities

### Startup/Auth

1. Show brand-forward first-open startup screen for signed-out users.
2. Use Clerk native auth widget as the actual login/sign-up surface.
3. Provide clean bootstrap loading state and a recoverable bootstrap failure state.

### Home

1. Home is a full-height assistant chat surface.
2. Support keyboard-first messaging with a persistent bottom composer.
3. Support optional voice input from the composer area (on-device transcription).
4. Keep listening/speaking indicators subtle and unobtrusive.

### Activity

1. Show chronological timeline of events.
2. Support filters (reminder, brief, urgent, system).
3. Support detail view for an event.

### Connectors

1. Show connected providers and connection health.
2. Show Google connection controls for v1.
3. Preserve extensible layout for future providers.

### Profile

1. Show account identity state.
2. Host preferences and notification settings.
3. Host privacy actions (revoke, delete-all) with explicit confirmations.

## 6) Visual System (Monochrome Cartoon, Dark-Only)

Core palette (four-tone baseline):

1. `ink`: near-black for app background and strong contrast.
2. `charcoal`: dark elevated surfaces and containers.
3. `smoke`: subdued text/secondary contrast.
4. `paper`: near-white for primary text and primary CTA fills.

Component styling rules:

1. Use semantic theme tokens; do not hard-code ad hoc colors in feature views.
2. Keep palette grayscale-only for Phase I (no hue accents unless explicitly approved).
3. Use thick outlines and hard shadows for cartoony depth.
4. Prefer bold typography weight for primary headlines and buttons.
5. Preserve strong text contrast and Dynamic Type support.

## 7) State UX Rules (Required)

Every async surface must support:

1. Loading state (skeleton/redacted placeholders where practical)
2. Empty state (clear message + next action)
3. Error state (human-readable message + retry)
4. Offline or transient failure state (recoverable with retry/backoff messaging)

Do not block the whole app with spinners for local section fetches.

## 8) SwiftUI Engineering Rules (Required)

This section is mandatory for all front-end issues.

1. Build composable views with clear responsibility boundaries.
2. Reuse shared components for repeated UI patterns (cards, rows, headers, state views).
3. Prefer modern SwiftUI data flow (`@State`, `@Binding`, `@Environment`, `@Observable`) over large ad hoc view models.
4. Keep files small and focused:
   1. Target `<= 500` lines for handwritten Swift files.
   2. If a file exceeds `500` lines, split in the same issue unless blocked.
5. Do not add new logic into already-large files without extracting subviews/helpers first.
6. Keep routing/state orchestration outside presentational subviews.
7. Add lightweight comments only where flow is non-obvious.

## 9) Skills Guidance For Agents

When implementing front-end issues, use the repository SwiftUI skills as applicable:

1. `swiftui-ui-patterns` for tab/navigation/sheet composition and general UI structure.
2. `swiftui-view-refactor` for view decomposition and dependency flow cleanup.
3. `swiftui-performance-audit` for rendering/performance risk review.
4. `swift-concurrency-expert` when async state/concurrency behavior is changed.

Agents should explicitly note skill usage in issue updates when a skill guided implementation decisions.

## 10) Acceptance Checklist For Front-End Issues

Before handoff:

1. `just ios-build` passes.
2. `just ios-test` passes when core logic/state behavior changed.
3. UI follows tab architecture and monochrome-cartoon theme token usage.
4. Loading/empty/error states are implemented for touched screens.
5. Files respect modularity and size constraints from this spec and `AGENTS.md`.

## 11) Relationship To Existing Docs

1. Product intent: `docs/product-context.md`
2. Engineering constraints: `docs/engineering-standards.md`
3. Execution board: `docs/phase1-master-todo.md`
4. Agent runtime protocol: `agent/start.md` and `AGENTS.md`
