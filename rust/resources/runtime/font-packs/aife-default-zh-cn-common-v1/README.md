# AI First Engine Default zh-CN FontPack

This engine-owned pack is generated from the pinned Noto Sans SC Regular source and the versioned glyph-set
inputs in this directory. The complete TTF is a development/build input and is never copied into a
RuntimePackage. Runtime packages receive only the sealed CookedFontBundle metadata and bitmap/MSDF pages.

Licenses and source identities are recorded in `provenance.json`. Do not replace either source without updating
its hash, glyph lock, recipe identity, cooked artifact, tests, and attribution.

The sealed v1 artifact contains 1,357 codepoints (1,223 Han), three 1024x1024 Bitmap R8 pages, and seven
1024x1024 MSDF RGBA8 pages. Noto Sans SC does not map U+FFFD, so the producer records a visible U+25A1
replacement alias with a deterministic synthetic glyph id. Rebuild intentionally with:

```powershell
Set-Location G:\gameEngin\rust
cargo run -p editor_core --bin build_builtin_font_pack
```

Glyph shard caching and MSDF parallel production are not part of this pack version.
