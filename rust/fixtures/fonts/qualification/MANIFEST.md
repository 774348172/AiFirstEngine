# Font Backend Qualification Fixtures

These files exist only to qualify the editor-side font parser, hinted bitmap
rasterizer, and MSDF generator. Runtime crates must not load them or depend on
the parser/rasterizer toolchain.

## Noto-derived fixtures

Upstream project: Noto CJK

- https://github.com/notofonts/noto-cjk
- License: SIL Open Font License 1.1 (`LICENSE-OFL-1.1.txt`)
- Source font: `NotoSansSC-VF.ttf`
- Source SHA-256: `763146584cf0710223441356b4395e279021b0806c196614377a7a0174ae074a`
- Source font: `NotoSerifSC-VF.ttf`
- Source SHA-256: `a4aed9985a5916fbf6690456f8732a9fccd517938e353165d4142b4f11a39280`
- Subsetting tool: FontTools 4.59.0
- Variable-font instancing: `wght=400` before subsetting
- Retained characters are U+0020, U+0030-U+0033, U+003F, U+0041, U+0053,
  U+0056, U+4E2D, U+66F2, and U+754C.

Generated files:

| File | Purpose | SHA-256 |
|---|---|---|
| `AifeNotoSansSCQualification-Regular.ttf` | TrueType outline and hinting qualification | `f70ecb32e5b312ba7bc724977352139a3f691566dc2491377be3828631c9fab2` |
| `AifeNotoSCQualification.ttc` | TrueType Collection face selection; face 0 is Noto Sans SC and face 1 is Noto Serif SC | `278c89270cd70c8b3c9f4b284b54bfe8639f8c271e5c4fdce7cc0d90251b0d75` |

## Project-owned fixture

`AifeQualificationCFF-Regular.otf` is a neutral CFF/OpenType qualification
fixture created for this repository. It contains neutral Latin sharp-corner
and curved outlines. See `LICENSE-PROJECT-QUALIFICATION.txt`.

SHA-256:
`b89d882e3082ac1f03c337af4e1ba3250384e9b4e645907a0e6f8e606ec3bf93`

## Reproducibility

Qualification tests pin these file hashes. Fixture replacement requires an
explicit manifest update and a new backend qualification result. Generated
font bytes are repository inputs; the build does not read fonts from the
operating system.
