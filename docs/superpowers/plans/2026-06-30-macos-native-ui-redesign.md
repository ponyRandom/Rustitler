# macOS Native UI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recompose Rustitler's existing frontend into a macOS-style utility shell without changing backend behavior.

**Architecture:** Keep the current single-file React view structure, but add a persistent sidebar and toolbar around the existing queue, history, and settings workflows. Use focused structural tests to drive the shell and workspace changes, then replace the stylesheet with a macOS native visual language while preserving existing accessibility roles and workflow labels.

**Tech Stack:** React 18, TypeScript, Vite, Vitest, Testing Library, and the existing single `src/App.css` stylesheet.

## Global Constraints

- Do not add true document preview rendering.
- Do not add new backend behavior, IPC commands, or processing states.
- Do not replace React/Vite/Tauri or introduce a third-party design system.
- Do not rework scoring, rename, history, or settings logic.
- Use a single system-blue accent, soft neutral surfaces, subtle borders, and minimal shadow.
- Keep edits scoped to `src/App.tsx`, `src/App.css`, `src/App.test.tsx`, and this plan document.

---

### Task 1: Shell Structure

**Files:**
- Modify: `src/App.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.css`

**Interfaces:**
- Consumes: existing `Tab` state and `QueueView`, `HistoryView`, `SettingsView` components.
- Produces: `Sidebar`, `Toolbar`, and `ShellView`-style markup through existing `App` render tree.

- [ ] **Step 1: Write failing shell test**

Add a test that expects:

```tsx
expect(screen.getByRole("navigation", { name: "主导航" })).toHaveClass("sidebar-nav");
expect(screen.getByRole("banner", { name: "应用工具栏" })).toHaveClass("toolbar");
expect(screen.getByRole("status", { name: "服务状态" })).toHaveTextContent("服务正常");
```

- [ ] **Step 2: Verify red**

Run: `npm test -- src/App.test.tsx -t "renders the macOS shell"`

Expected: FAIL because the current navigation is still in the top bar and no toolbar banner or service status exists.

- [ ] **Step 3: Implement minimal shell**

Move navigation into a sidebar, add a toolbar with title/summary/actions, and keep existing tab switching.

- [ ] **Step 4: Verify green**

Run: `npm test -- src/App.test.tsx -t "renders the macOS shell"`

Expected: PASS.

### Task 2: Queue Workspace

**Files:**
- Modify: `src/App.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.css`

**Interfaces:**
- Consumes: `QueueView` props and existing import/cancel handlers.
- Produces: toolbar import actions, queue master list, and right inspector using existing `FileDetail`.

- [ ] **Step 1: Write failing queue workspace test**

Add a test that expects queue content to expose:

```tsx
expect(screen.getByRole("region", { name: "队列工作区" })).toHaveClass("queue-layout");
expect(screen.getByRole("region", { name: "文件队列" })).toHaveClass("queue-panel");
expect(screen.getByRole("complementary", { name: "文件检查器" })).toHaveClass("detail-panel");
expect(within(screen.getByRole("banner", { name: "应用工具栏" })).getByRole("button", { name: "导入文件" })).toBeEnabled();
```

- [ ] **Step 2: Verify red**

Run: `npm test -- src/App.test.tsx -t "uses a macOS queue workspace"`

Expected: FAIL because the current queue section lacks the region labels and toolbar import buttons.

- [ ] **Step 3: Implement queue workspace**

Add region labels, move the main import/cancel actions to toolbar props, and keep import buttons in the empty/drop state as secondary entry points.

- [ ] **Step 4: Verify green**

Run: `npm test -- src/App.test.tsx -t "uses a macOS queue workspace"`

Expected: PASS.

### Task 3: History and Settings Structure

**Files:**
- Modify: `src/App.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.css`

**Interfaces:**
- Consumes: existing `HistoryView` and `SettingsView` behavior.
- Produces: labeled history master-detail layout and grouped settings panes inside the shared shell.

- [ ] **Step 1: Write failing structure test**

Add assertions that history detail uses `aria-label="批次检查器"` and settings scoring/rules groups use `aria-label="评分设置"` and `aria-label="关键词规则"`.

- [ ] **Step 2: Verify red**

Run: `npm test -- src/App.test.tsx -t "keeps history and settings in macOS groups"`

Expected: FAIL because the current panels do not expose the new labels.

- [ ] **Step 3: Implement labels and grouping**

Add the missing labels without changing command behavior.

- [ ] **Step 4: Verify green**

Run: `npm test -- src/App.test.tsx -t "keeps history and settings in macOS groups"`

Expected: PASS.

### Task 4: Visual Pass and Full Verification

**Files:**
- Modify: `src/App.css`

**Interfaces:**
- Consumes: class names introduced in Tasks 1-3.
- Produces: macOS-style layout and responsive behavior.

- [ ] **Step 1: Apply macOS visual language**

Use sidebar, toolbar, grouped panes, system-blue selected state, cool gray background, and stable two-column desktop layouts.

- [ ] **Step 2: Run focused UI tests**

Run: `npm test -- src/App.test.tsx`

Expected: PASS.

- [ ] **Step 3: Run full frontend verification**

Run: `npm test && npm run build`

Expected: PASS.
