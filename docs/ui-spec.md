# Alfred UI Specification (Phase I)

- Last Updated: 2026-02-15
- Scope: iOS front-end design and implementation rules for Phase I
- Audience: iOS engineers and autonomous coding agents

## 1) Source Of Truth

This document is the source of truth for iOS UI/UX implementation in Phase I.

If a front-end issue conflicts with ad hoc styling choices, follow this spec and update the issue notes.

## 2) Product Direction

1. Dark-mode-only experience for Phase I.
2. Native iOS bottom-tab navigation as primary app structure.
3. Home is the primary interaction surface.
4. Connectors must be first-class and future-extensible (Google now, others later).

## 3) Navigation Architecture

Primary tabs:

1. Home
2. Activity
3. Connectors
4. Profile

Architecture rules:

1. Use `TabView` at app root.
2. Use one `NavigationStack` per tab (independent history).
3. Route deep links through centralized router logic, not per-screen ad hoc handlers.
4. Keep tab identity centralized in one enum.

## 4) Screen Responsibilities

### Home

1. Show actionable summary of the day.
2. Show reminder/brief/urgent status cards.
3. Expose quick actions (refresh, retry, open connector state).

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

## 5) Visual System (Dark-Only)

1. Use near-black base surfaces and elevated dark cards.
2. Use semantic colors/tokens; avoid hard-coded ad hoc colors in feature views.
3. Keep one primary accent color for core actions.
4. Preserve strong text contrast and Dynamic Type support.

## 6) State UX Rules (Required)

Every async surface must support:

1. Loading state (skeleton/redacted placeholders where practical)
2. Empty state (clear message + next action)
3. Error state (human-readable message + retry)
4. Offline or transient failure state (recoverable with retry/backoff messaging)

Do not block the whole app with spinners for local section fetches.

## 7) SwiftUI Engineering Rules (Required)

This section is mandatory for all front-end issues.

1. Build composable views with clear responsibility boundaries.
2. Reuse shared components for repeated UI patterns (cards, rows, headers, state views).
3. Prefer modern SwiftUI data flow (`@State`, `@Binding`, `@Environment`, `@Observable`) over large ad hoc view models.
4. Keep files small and focused:
   1. Target `<= 300` lines for handwritten Swift files.
   2. Hard ceiling is `500` lines. If exceeded, split in the same issue unless blocked.
5. Do not add new logic into already-large files without extracting subviews/helpers first.
6. Keep routing/state orchestration outside presentational subviews.
7. Add lightweight comments only where flow is non-obvious.

## 8) Skills Guidance For Agents

When implementing front-end issues, use the repository SwiftUI skills as applicable:

1. `swiftui-ui-patterns` for tab/navigation/sheet composition and general UI structure.
2. `swiftui-view-refactor` for view decomposition and dependency flow cleanup.
3. `swiftui-performance-audit` for rendering/performance risk review.
4. `swift-concurrency-expert` when async state/concurrency behavior is changed.

Agents should explicitly note skill usage in issue updates when a skill guided implementation decisions.

## 9) Acceptance Checklist For Front-End Issues

Before handoff:

1. `just ios-build` passes.
2. `just ios-test` passes when core logic/state behavior changed.
3. UI follows tab architecture and dark-theme token usage.
4. Loading/empty/error states are implemented for touched screens.
5. Files respect modularity and size constraints from this spec and `AGENTS.md`.

## 10) Relationship To Existing Docs

1. Product intent: `docs/product-context.md`
2. Engineering constraints: `docs/engineering-standards.md`
3. Execution board: `docs/phase1-master-todo.md`
4. Agent runtime protocol: `agent/start.md` and `AGENTS.md`
