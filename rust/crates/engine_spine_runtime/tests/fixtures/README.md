# Gate A fixtures

`probe.json` and `probe.atlas` are project-authored mathematical test data, not
Spine Editor exports or copied example assets. They exercise the real official
4.2 JSON/atlas parsers. `4.2.00` is the declared test-format version, not a claim
about an Editor installation. The triangle has coordinates (0,0), (2,0), (0,2).
No PNG is required: Gate A reads atlas metadata with texture creation disabled.
This is not rendering or production import qualification.

The source animation deliberately moves BOTH lower and upper bones. A mask made
from it must exclude the lower translation and retain upper translation/rotation
and deform, with fresh timeline ownership and canonical attachment identity.

These synthetic fixture files may be used and redistributed with this project.
The separately vendored official runtime retains its own Spine Runtimes License.
