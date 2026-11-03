# Independent glTF decoding fixture: Box

Source: [KhronosGroup/glTF-Sample-Assets, Models/Box](https://github.com/KhronosGroup/glTF-Sample-Assets/tree/81e8b567643b5166e6ff40024e4ff71ad4b18676/Models/Box).
Pinned upstream commit: `81e8b567643b5166e6ff40024e4ff71ad4b18676`.

Copyright 2017 Cesium. Licensed under [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/legalcode).
The original attribution is retained in `UPSTREAM-README.md` and `metadata.json`.
`provenance.json` records upstream URLs, Git blob identities, byte lengths and SHA-256 hashes.

The glTF JSON, external binary buffer and GLB are copied byte-for-byte, without any conversion by this engine. The source declares `COLLADA2GLTF` as its generator; its exact historical exporter flags are not provided upstream and are not invented here. Source settings include a two-node hierarchy, an explicit root matrix, one PBR material, FLOAT position/normal data, a 12-byte source stride, and 16-bit triangle indices.

A2.1 tests decode the original 24 vertices and 36 indices, compare the glTF and GLB geometry, and verify independently known cube bounds and unit normals. These are source-space bytes. This fixture does not qualify PBR material conversion, node matrix conversion, prefab publication or rendered appearance; those remain A2.2 work. No modifications were made to fit a material subset.

The neighboring owner tests create exact small fixtures directly from glTF JSON and little-endian source bytes. They do not use the importer as an exporter.
