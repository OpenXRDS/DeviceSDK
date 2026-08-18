# Android asset cache — per-build stamp

**Shipped.** Replaces a per-file size comparison that silently kept stale assets on
device.

## The bug

`android_main`'s APK-to-cache extractor skipped a file when
`cached_size == Some(bytes.len())`. Any edit that preserved byte length — a float
tweaked in `scene.json`, a recoloured PNG, a same-length string — left the device
running the old asset.

Reinstalling did not help: the cache lives in app data, not the APK. And Android does
not clear app data on APK upgrade, so **this was a shipping bug, not only a dev-loop
annoyance** — an update whose asset changed at constant size would never reach existing
users. `adb shell pm clear` was a workaround only developers had.

The check was also in the wrong place to save what it looked like it saved:

```rust
let mut bytes = Vec::new();
f.read_to_end(&mut bytes)?;                                  // whole asset, always
let cached_size = std::fs::metadata(&dest).ok().map(|m| m.len());
if cached_size == Some(bytes.len() as u64) { continue; }      // decided afterwards
```

Every launch already opened, decompressed and read all ~142 MB out of the APK. The
skip avoided only the `fs::write`, so the expensive half was unconditional.

## The fix

`build.sh`/`build.ps1` emit `ASSET_STAMP`, a fresh UTC timestamp per build.
`android_main` compares it against the cached copy:

- **Match** → skip the extraction loop entirely, including the APK reads. This makes
  warm launches *faster* than the code it replaced.
- **Mismatch** → extract everything unconditionally, and write the stamp **last**.
- **No stamp in the APK** (pre-existing build) → extract every launch rather than trust
  a cache that cannot be dated.
- **Any file failed** → do not stamp, so the next launch retries.

The stale stamp is removed *before* extraction begins, so a crash or a kill mid-pass
cannot leave a matching stamp over a half-populated cache.

Extraction now streams via `io::copy` instead of `read_to_end` into a `Vec` — the
largest bundled assets run to tens of MB and buffering one whole file was a needless
peak on a memory-constrained headset.

### Why a timestamp rather than a content hash

A hash would avoid re-extracting after a no-op rebuild, but it can false-match, and
false-matching is exactly the failure being fixed. A timestamp cannot. The cost is one
extra extraction after a rebuild that changed nothing; the benefit is that new work
always reaches the device. That trade was made deliberately.

## Verified on device — Quest 3, four runs

| Run | Setup | Result |
|---|---|---|
| 1 | `pm clear`, cold | `15 written, 0 failed` |
| 2 | relaunch, same build | `assets already extracted (stamp build=20260812T074844Z) — skipping` |
| 3 | **byte-length-identical scene edit, no `pm clear`** | re-extracted → **`5 entities`** |
| 4 | scene restored | back to `4 entities` |

Run 3 is the proof. `res/default.json` was rewritten from 4419 bytes to 4419 bytes
exactly — JSON compacted, one node added, padded with trailing whitespace — then
rebuilt, installed and launched with no `pm clear`. It reported 5 entities. The old
code would have reported 4.

Zero panics across all four runs.

## Follow-on findings

Both are recorded in `docs/quest-device-test-recipe.md`:

- **Trap 7 in that recipe was wrong.** The raw `cargo ndk` command it recommended as
  the Android health check cannot work — `build.ps1` exports
  `CMAKE_GENERATOR=Ninja` + `CMAKE_MAKE_PROGRAM` first, without which CMake picks a
  Visual Studio generator and quiche's vendored BoringSSL fails with
  `clang.exe - broken`. Now points at the build script.
- **A failed CMake configure poisons the cache.** `CMakeCache.txt` persists in the
  target dir and every later attempt fails identically even after the environment is
  fixed, until `target/**/build/quiche-*` is deleted.
