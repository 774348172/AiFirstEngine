---
name: tower-defense-project
description: Project rules for designing, implementing, testing, or building the tower defense game under G:\gameEngin\samples\tower_defense_project. Use for tower-defense planning, Feature Specs, data contracts, Project Assets, Gameplay Rules, RuntimeModule logic, AUI, scenes, tests, validation, and builds.
---

# Tower Defense Project Rules

Treat this game as an external user project made with the engine, not as an engine-development
vehicle. Also load `ai-first-game-engine-project` for repository-wide architecture and workflow
rules.

## Hard Boundary

- Keep all tower-defense designs and implementations within
  `G:\gameEngin\samples\tower_defense_project`.
- Use only engine capabilities available to a normal project author: project manifests, settings,
  assets, scenes, prefabs, Gameplay Rules, AUI, input, build profiles, and project-owned Rust
  Framework / RuntimeModule interfaces.
- Do not modify engine source, editor source, shared engine schemas, repository-wide build
  infrastructure, or generic engine tests to make a tower-defense feature work.
- Do not propose an engine modification as part of a tower-defense plan unless the user has
  explicitly approved modifying the engine.
- Treat statements such as "继续制作", "修好它", or approval of a tower-defense plan as project
  authorization only. They do not authorize engine changes.

## Engine Capability Gaps

When the requested game behavior cannot be implemented through the current public project surface:

1. Stop before changing engine-owned files.
2. Identify the missing engine capability and provide concrete evidence.
3. Explain the affected tower-defense requirement.
4. Offer a project-only fallback or reduced-scope simulation when one is viable.
5. Ask for explicit permission before planning or applying an engine change.

Require a clear instruction such as "允许修改引擎代码来补充该能力". Silence, implied necessity,
or convenience is not approval.

After explicit approval, separate the engine change from the tower-defense project work and follow
the repository's normal engine discussion, confirmation, construction, and validation workflow.

## Project Workflow

1. Read the current files under `samples/tower_defense_project` before making assumptions.
2. Keep gameplay-specific concepts such as units, waves, military grain, merging, heroes, and
   augments in project-owned data or logic.
3. Prefer schema-first, deterministic, versioned project contracts and stable IDs.
4. Put configurable content in Project Assets / Gameplay Rules and complex deterministic behavior
   in the project-owned Rust Framework / RuntimeModule.
5. Validate through the same public project and runtime seams available to engine users.
6. Report any workaround forced by an engine limitation; do not disguise it as completed native
   support.

## Scope Check

Before every proposed file change, classify each target as:

- `project-owned`: allowed when it is required by the user's tower-defense request.
- `engine-owned`: read-only until the user explicitly authorizes an engine change.
- `unclear`: inspect ownership and references first; do not write until classified.

If a plan contains any `engine-owned` target without explicit authorization, revise the plan to
remain project-only or pause for user approval.
