---
name: UnoSkills
description: Design standards for the native desktop workbench.
---

# UnoSkills Desktop Design

## Overview

UnoSkills is a native desktop workbench for managing skills across agents and projects. The interface keeps the current scope, selected item, and next action visible. Use a consistent shell and compact, readable controls across Home, Library, Discover, Projects, Stacks, and Settings.

Use Native Shell (NSH) v0.6.1 elements and compose shared controls in `crates/unoskills-app/ui/src/components/`. The TSX layer owns appearance and interaction. Rust owns application data and operations. [Architecture and placement rules](docs/ARCHITECTURE.md)

## Colors

The six presets in `crates/unoskills-app/ui/themes/` are the source of truth for color. Each preset supplies shadcn-format `cssVars.light` and `cssVars.dark`. `theme.ts` maps the current set to TSX components and passes the same sets to NSH. In System mode the app follows OS appearance changes. A fixed Light or Dark choice selects that set immediately.

| Surface or state | Token pair |
| --- | --- |
| Page and standard text | `background` and `foreground` |
| Cards and detail panels | `card` and `card-foreground` |
| Menus, dropdowns, and dialogs | `popover` and `popover-foreground` |
| Primary buttons and the current navigation item | `primary` and `primary-foreground` |
| Selected segments in settings | `secondary` and `secondary-foreground` |
| Selected rows and dropdown options | `accent` and `accent-foreground` |
| Sidebar and its unselected items on hover | `sidebar` and `sidebar-foreground` at rest, `sidebar-accent` and `sidebar-accent-foreground` on hover |
| Quiet hover fills and secondary text | `muted` and `muted-foreground` |
| Destructive buttons | `destructive` and `destructive-foreground` |
| Separators, input outlines, and focus | `border`, `input`, and `ring` respectively. Sidebar focus uses `sidebar-ring`. |

Keep each surface's text and icons with its paired foreground. Use the same state rules for every preset. A primary button can dim on press using its own color, and a selected navigation item keeps its fill on hover. The shared warning and success colors indicate outcomes such as conflicts and verified deployments. Include a label or accessible icon name for each outcome.

Treat contrast as a property of the rendered color pair. Record weak pairs during review. Make any approved palette changes in the preset itself, where TSX and NSH can use the same values.

## Typography

Use shared `Label` and `textStyle`. The default body size is 14 points, with page titles around 20 and supporting text around 12. `textSize` scales supporting sizes with the base setting. Standard labels use a line height of 1.4 times the font size, rounded up to the next point. Give rows enough height for the largest supported text size and CJK font fallbacks.

Auto starts with the preset font and uses language-appropriate fallbacks. An explicit font choice takes precedence. Use the configured monospace family for paths, source editing, and code blocks. Truncate only where the complete value remains available through a tooltip, detail view, or accessible name.

## Layout

- The shared title bar is 36 points high. The sidebar is 184 points wide when expanded and 48 when collapsed. It collapses below a window width of 1080 points. Keep the page title and action in the shared `PageHeader`.
- Use 20 points of horizontal padding for main content and gaps of 4, 8, 12, 16, or 24 points where they match the surrounding controls. Align text to a common inset. Keep the current object and action visible when a pane scrolls.
- Use a constrained viewport for scrolling and NSH virtualized lists for growing datasets. Keep paths and other long values in shrinking columns so controls remain usable. Review at the 960 × 600 minimum and the 1240 × 780 reference size.
- Home provides local workspace shortcuts and up to six skills.sh entries. Short windows start with three entries and expose a control to expand the list. Use local loading feedback where each result will appear.

## Elevation & Depth

Separate the rail, page, cards, and transient popups with the preset's surfaces. Use `border` for structural separators. Shared controls, panels, and popups use the preset's `shadow-xs`, `shadow-sm`, and `shadow-md` respectively. Dialogs use the common dimming backdrop.

## Shapes

Use the preset's radius when the radius setting is Auto. Controls use the preset radius minus two points, clamped at zero. Panels use the preset radius plus four points. An explicit radius setting applies to preset-based controls and panels. Switches retain the pill track and round thumb that indicate their on/off behavior.

## Components

| User intent | Shared control and state rules |
| --- | --- |
| Navigate between pages | `NavigationControl` sets the current page's primary fill and uses the relevant accent pair for hover on other items. Keep the keyboard outline visible. |
| Switch content within a page | `TabList` or `PageTabs` uses NSH `Tabs` and its moving underline. Settings segments choose a preference value. |
| Run an action | Use `Action` or `ActionControl`. The primary style identifies the main action. Pending work disables repeat submission and shows feedback on the control or in its result area. |
| Edit or choose a value | Use `TextInput`, `FormText`, or `SelectControl`. Give searchable dropdowns a bounded list, retain keyboard focus behavior, and keep placeholder and selected-item styles distinct. |
| Change an on/off setting | Use `ToggleSwitch` over NSH `Switch`. The on track uses `primary`. The thumb uses `background` in light mode and `primary-foreground` when on or `foreground` when off in dark mode. |
| Select one or more items | Use `CheckControl` over NSH `Checkbox`. The unchecked outline uses `input`. The checked fill and mark use the primary pair. |
| Present a menu or confirmation | Reuse `ActionMenu`, `Modal`, or an NSH `AlertDialog`. Use the shared popup surface for custom dialog layouts and keep destructive confirmation explicit. |
| Read or edit a skill | Use NSH `TextView` for Markdown and `Editor` for source. Respect configured typography and keep editor focus tied to the visible skill. |

Use shared 120 ms color transitions for control feedback. Let NSH handle tab indicator movement and dialog animation. Honor reduced motion and keep page changes immediate. Show a skeleton where the first load has a known shape, retain existing results during refresh, and place errors next to the affected operation.

## Do's and Don'ts

- Read theme pairs directly from the active preset. Keep any component-specific color rules in the shared component and apply them across presets.
- Use named controls with visible keyboard focus. Test the control at rest, on hover, on press, when focused, selected, and disabled where applicable.
- Verify Light, Dark, and System behavior, including a live appearance change with the page open. Check long labels in English, Simplified Chinese, and Japanese, and the largest text size.
- Use native app captures to review affected workflows and headless tests to cover repeatable state changes. Use temporary data when testing filesystem changes. [Verification steps](CONTRIBUTING.md) · [NSH v0.6.1 design guide](https://github.com/longbridge/nsh/blob/v0.6.1/website/zh-CN/docs/design-guides.md)
