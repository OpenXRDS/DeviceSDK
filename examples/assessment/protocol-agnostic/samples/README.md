# Sample data

The content transferred by the protocol-agnostic assessment: exactly one file
per supported XRDS content type, gathered here so the assessment travels as a
single directory rather than as references into a full SDK checkout.

One file per type is the rule. The transports move opaque bytes and verify a
SHA-256, so a second file of the same type exercises no new code — it only
lengthens every protocol's run.

## The default suite

These five are what `assess_*_sender` transfers when no `--file` / `--files` is
given, one per content type in the vocabulary:

| File | Content type | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `StainedGlassLamp.gltf` | `gltf-model` | 30,985 | `ddebbf019fabb7c870de0ac222f170279d6213965b40f2fb47ae0a3ecc3dd59c` |
| `arrow_box.png` | `texture` | 2,217 | `d4f69954a85057015a2743152186a83cd08c98ecda029736ecca5bd9478cee86` |
| `spatial_test_ping.wav` | `audio` | 88,244 | `15bcfd9c3b0296eb393946b267048ccae313ec4d0ea9050622c0e3f53c2a16c7` |
| `tiny_test.mp4` | `video` | 2,943 | `ef351e09c58c9bbafa13b80defed6c642d66dcbcfde0999f3b01b2c287141544` |
| `assessment_scene.xrds` | `scene-document` | 4,002 | `080f65abad4abbb2e1a2d380a31fca843e9527c8a7847222e9f1add9cb75d3e4` |

The hashes are here so a conductor can confirm the files they received are the
files this suite was built from, independently of the per-run integrity check
the tools perform.

They are small on purpose. Every protocol has to get through the whole suite in
a sitting, and MQTT opens one broker connection per chunk. Throughput is a
separate question — swap in something bigger with `--files` when that is what
you are measuring.

That is the whole directory: five files, one per content type, 128 KB in total.

## Read this before judging the glTF result

`StainedGlassLamp.gltf` is the glTF **document** only. Its geometry
(`StainedGlassLamp.bin`, 1.8 MB) and its twenty PNG textures (~37 MB) are
sibling files in the original asset folder, and they are deliberately **not**
part of this item. The received file is therefore not loadable on its own.

That is not a gap in the result. This assessment moves opaque bytes and proves
they arrive intact; it makes no claim about what the bytes mean. A
self-contained `.glb` would travel the identical code path — the transports
never inspect the payload — so carrying one as well would add a second row
proving the same property twice. Whether an arrived model actually opens is a
question about the SDK's glTF loader, and it is answered by the reference
examples and the loader's own unit tests.

## Provenance and licensing

`StainedGlassLamp.gltf` is © 2021 Wayfair LLC, released under
[CC-BY 4.0](https://creativecommons.org/licenses/by/4.0/). It is a Khronos glTF
sample model. **Attribution is required if this file is redistributed**, which
includes handing this directory to a third party.

`assessment_scene.xrds` is produced by the SDK's own
`XrdsSceneDocument::save_json` — a root, a cube with a material, a point light
and a camera. It is a real document from the serializer, not a hand-written
stand-in, which is the point: the scene-document row of the matrix has to
exercise the format the SDK actually writes.

The provenance of `arrow_box.png`, `spatial_test_ping.wav` and `tiny_test.mp4`
is not recorded in this repository. They are small internal test fixtures and
have been treated as such. **Confirm their origin and licensing before
distributing this directory outside your organisation** — that check has not
been done here, and nothing in the repo settles it.

## Where this directory has to be

The default suite is resolved at run time, not baked into the binaries. The
tools look for `samples/` in three places, in order:

1. beside the running executable — the portable kit layout;
2. `samples/` under the working directory;
3. `examples/assessment/protocol-agnostic/samples/` under the working
   directory — a checkout, run from the repo root as the README describes.

So one build serves both a checkout and a kit copied onto a machine with no
toolchain. The practical consequence: **the sender kit must carry a copy of
this directory beside its executables.** The binaries hold no content of their
own, and a sender with no `samples/` fails at the first item.

Paths passed with `--file` / `--files` are used exactly as typed and are not
searched for.

## Replacing these files

Nothing depends on these specific files; the suite is a table in
`assess_common.rs` (`DEFAULT_SUITE`). If you substitute your own content, keep
one file per content type so the matrix stays complete, update the table, and
re-record the hashes above. Content type is inferred from the extension, so a
substitute with a known extension needs no other change.
