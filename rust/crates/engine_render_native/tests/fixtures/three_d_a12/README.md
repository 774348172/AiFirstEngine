# 359 A1.2 generated Cook fixtures

These five files are exact `Compiler.prepare` Cook outputs, captured from ordinary
project inputs by `project_authoring_execution/tests/three_d_project_a/generated.rs`.
That test now compares every byte against the real compiler on each run. It also
checks dimensions, normals, triangle winding and plane UV direction independently.
The runtime GPU test consumes these files through RuntimePackageBuilder and the
existing package loader / Prepare owner; runtime contains no primitive generator.

- `cube`: centered cube, full size `[1, 1, 1]`.
- `wide`: centered cube, full size `[2, 1, 1]`.
- `plane`: XZ plane, full size `[2, 4]`, normal +Y, U toward +X, V toward +Z.
- `blue`: opaque Unlit linear blue, back cull, depth test/write, no texture.
- `textured`: white Unlit, `guid-direction` base-color texture, nearest min,
  linear mag, repeat S, mirroredRepeat T. Prepare samples base-color RGB as sRGB.

All source fixtures are repository-authored. Direction texture pixels and all
additional test-only sampling/cull/color variants are explicit in the tests.
These are source-level offscreen GPU fixtures, not installed/window qualification.
