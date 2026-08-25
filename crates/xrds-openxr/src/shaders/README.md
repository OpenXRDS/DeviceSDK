# SPIR-V shaders

Compiled ahead of time and committed alongside their GLSL source, so building this
crate needs no Vulkan SDK. Only someone *editing* a shader needs `glslc`:

```shell
glslc --target-env=vulkan1.1 -O video_convert.vert -o video_convert.vert.spv
glslc --target-env=vulkan1.1 -O video_convert.frag -o video_convert.frag.spv
```

Commit the `.spv` with the source it came from — a stale pair is silent, because
nothing at build time checks that one matches the other.

`video_convert.*` is the Android video conversion pass: it samples an imported
`AHardwareBuffer` through a Ycbcr-converting sampler and writes ordinary RGBA that
wgpu can then own. See `docs/video-asset-spike.md`.
