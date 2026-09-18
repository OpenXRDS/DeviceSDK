# Reference examples: count, build, run

The second XRDS assessment. The first one ([`../protocol-agnostic/`](../protocol-agnostic/))
asked whether XR content survives a round trip over five protocols. This one asks
a plainer question about the SDK itself:

> Does the SDK ship enough working reference source, and does it actually work?

"Enough" is a counting problem. "Actually works" is two problems — whether the
code compiles, and whether the resulting program runs — and they are answered
separately, because an example that builds and then dies on launch is not a
reference anyone can learn from.

Run it:

```powershell
pwsh -File run-references.ps1        # count, build, run -> runs/<run-id>/report.txt
pwsh -File make-contact-sheet.ps1    # all the screenshots -> runs/contact-sheet.png
```

The first builds every example, stages the binaries, launches each one,
photographs the ones that open a window, and writes the report. The second
turns those screenshots into one image you can put in front of people.

It needs a Windows machine with a real GPU and an **unlocked, interactive
desktop** — 57 of the 62 open a window, and the runner brings each one to the
foreground to photograph it, so don't use the machine while it runs. Budget
around 40 minutes, plus a cold `cargo build` on a fresh checkout. Some `net_*`
examples reach the public internet.

## The three claims

### 1. Count

The inventory is generated from the workspace `Cargo.toml`'s `[[example]]`
table, which is the only authoritative list — Cargo's example autodiscovery
does not recurse into `examples/<category>/<name>.rs`, so every example here
must have an explicit entry or it cannot be run at all.

The generated list is then cross-checked against the files on disk in both
directions:

- a `.rs` file under `examples/` with **no** `[[example]]` entry is reported as
  an orphan — source nobody can launch is not a reference;
- an entry pointing at a **missing** file is reported too.

Either one is a finding in the report, not a silent omission. The report also
flags examples with no entry in [`examples/README.md`](../../README.md), since
an undocumented example is a weaker reference than a documented one.

The ten `assess_*` examples under [`../protocol-agnostic/`](../protocol-agnostic/)
are excluded throughout. They are the *other* assessment's apparatus, not
reference code for SDK users.

### 2. Build

`cargo build --examples`, with `--message-format=json` parsed for the produced
artifact of each example and for any compiler errors. The report states how many
binaries were produced out of how many were declared. This claim is unambiguous:
either the compiler emitted a binary or it did not.

### 3. Run

This is the claim that needs machinery, and it is worth being explicit about
why. Most of these examples open a window and run until a human closes them.
They have no exit code, so "it ran" cannot be read off the process. Waiting ten
seconds and observing that nothing crashed yet is weak evidence.

Two mechanisms make it real.

**The frame-limit hook.** `XRDS_EXIT_AFTER_FRAMES=<n>` is an opt-in hook in
`xrds-runtime` (see `install_frame_limit` in `crates/xrds-runtime/src/runtime.rs`).
When set, the app renders that many frames, prints

```
XRDS_FRAME_LIMIT_REACHED frames=600 elapsed_ms=12043 avg_fps=49.8
```

and exits 0. That converts "it didn't crash" into a hard, machine-checkable
claim: *this program rendered 600 frames and exited cleanly*. The hook lives in
`Runtime::run`, so it covers every example that goes through `Runtime` —
including the expert `RuntimeHandler` path — and does nothing at all when the
variable is unset. Without it, the whole run stage would be guesswork.

**The screenshot.** While the app is up, the runner brings its window forward
and copies the pixels off the desktop into `window.png`. A GPU-composited
Vulkan surface is not reliably readable through `PrintWindow`, but it is always
readable off the screen. This is the evidence a human can check by eye, and for
twenty of these examples there is a reference image in
[`examples/screenshots/`](../../screenshots/) to compare against.

So that a photograph identifies itself, the runner sets `XRDS_WINDOW_TITLE` to
the example's name and the runtime uses it as the window title — the screenshot
then says `skybox`, not `OpenXRDS`. Like the frame hook it lives in
`build_bevy_app`, does nothing when unset, and does not reach the two raw-Bevy
examples that never construct a `Runtime`.

A windowed example that produced no photograph is reported *with the reason*
(`exited before a window appeared`, `window closed during the settle`, and so
on) rather than quietly counted as unphotographed.

**Quiet captures, measured but not judged.** Each capture is scored for
luminance spread, and a flat-looking one is re-photographed up to four times,
three seconds apart, so a slow asset load is not mistaken for an empty scene.
The six quietest are listed in the report for a person to open.

The runner deliberately stops short of turning that score into a pass/fail.
Both a stddev and a percentile-range metric were tried, and each ranked a
genuinely empty window *above* `load_gltf`, which draws a small dark lamp
against a dark background. `video_texture_check` makes the same point from the
other side: its clip has dark passages, so a capture that lands on one looks
like a dead texture when playback is perfectly healthy. An automatic verdict
manufactures findings that are not real, which is worse than no verdict — so
the number is reported and the judgement is left to a reader.

## Staging, and why it matters

The binaries are copied into `runs/_stage/` together with the native DLLs they
link — FFmpeg's `avcodec-61.dll` and friends from `target/debug/build/*/out/`,
plus `quiche.dll`. `cargo run` puts those directories on `PATH` for you; a bare
`target/debug/examples/foo.exe` gets nothing and dies immediately with
`STATUS_DLL_NOT_FOUND` (exit `-1073741515`).

Staging is both the fix and a stronger test. Windows resolves a DLL from the
executable's own directory first, so a staged binary runs on its own — which is
what "the example works" ought to mean.

Staging has a sharp edge worth knowing about, because it cost a whole run
before it was spotted. **Bevy resolves its asset root from the executable's own
directory, not from the working directory.** A staged binary therefore looks for
`assets/` *inside the stage*, finds nothing, logs `Path not found` at `ERROR`
level — and carries on rendering an empty scene, exiting 0 with a full frame
count. Twelve examples "passed" that way before the blank-window check caught
them. The runner now sets `BEVY_ASSET_ROOT` to the repo root on every launch,
which is why that check earns its place: it is the only signal that
distinguishes a program that works from one that merely runs.

## How each example is driven

The runner classifies every example by reading its source, so a newly added
example is handled without editing the script:

| Detected in source | Mode | Passes when |
| --- | --- | --- |
| `run_xrds` / `Runtime::new` | `frames` | exits 0 under the frame-limit hook; a frame count and usually a screenshot are recorded |
| `App::new()` (raw Bevy, no XRDS runtime) | `window-soft` | a window appeared and was photographed, with no panic — there is no hook to give these a clean exit |
| `#[tokio::main]` | `service` | still running and producing output at the threshold, or exited 0 on its own |
| anything else | `oneshot` | exits 0 within the timeout |

Not every example has a window, and for those the terminal *is* the result. The
`oneshot` and `service` examples get a **TERMINAL EVIDENCE** section in the
report carrying the first meaningful lines of what they printed — so
`net_protocols`' per-protocol verb results or `net_app_publisher`'s `PubAck`
are visible in the report itself, not only in the evidence folder. Full
`stdout.txt` and `stderr.txt` are kept for every example either way.

Every example is expected to run successfully. There is no "expected to fail"
category and no opt-out: an example that demonstrates a failure path belongs in
the unit tests, not in the example pool, so a red status here is always a real
finding.

Three cases source inspection cannot see are named explicitly in
`$ModeOverrides` at the top of the script:

- `net_app_publisher` publishes in an infinite loop by design → `service`;
- `webrtc_file_stream` serves until stopped → `service`;
- `webrtc_webcam_stream` needs a real camera and microphone → `hardware`, which
  is **skipped by default**. On a machine without a camera its outcome says
  nothing about the SDK, and counting it either way would distort the pass rate.
  The report names it as skipped, with the reason. Pass `-IncludeHardware` on a
  machine that does have the hardware, and it is assessed into its own
  `HARDWARE-OK` / `HARDWARE-UNAVAILABLE` bucket, still outside pass/fail;
- the three `webrtc_realnet_*` binaries are a three-process handshake, so they
  are driven as one scenario: start the signaling server, start the publisher,
  scrape the session id it prints, join with the subscriber.

## What this assessment does not prove

- **Correctness.** A green run means the program started, drew something and
  exited cleanly. The blank-window check rules out an *empty* picture; it says
  nothing about whether the picture is the *right* one. The screenshots exist so
  a human can make that judgement; the runner does not make it.
- **Non-Windows targets.** The runner is PowerShell and uses Win32 window
  capture. The count and build claims hold anywhere; the run claim, as
  automated here, is Windows-only.
- **Headless machines.** 57 of the 62 examples open a window and need a real
  GPU and an unlocked interactive desktop. Over RDP or on a headless box they
  will fail for reasons that have nothing to do with the SDK.
- **The network-dependent examples.** Several `net_*` examples reach the public
  internet (`rust-lang.org`, `test.mosquitto.org`). They assess the SDK *and*
  the connection the machine happened to have.

## Binary fingerprints

Every run writes `binaries.csv`: the SHA-256 of each of the 62 executables,
with its size and build timestamp.

```csv
example,file,bytes,modified_utc,sha256
simple_api,simple_api.exe,169619456,2026-09-16T07:24:30Z,7B9420D6FAF77DBD...
```

A report saying "62 examples passed" is only checkable if you can tell whether
the binary in front of you is the binary that was run. The hashes are taken
from the staged executables at run time rather than at build time, so they
describe what actually executed — including on a `-SkipBuild` pass, where the
build section has nothing to say but the binaries still need identifying.

Native DLLs are deliberately not hashed. They are third-party libraries copied
in unchanged, and they are not what this assessment makes a claim about.

Hashing all 62 costs a few seconds (about 8 GB of debug binaries), so it runs
unconditionally.

## The contact sheet

Forty-eight screenshots in forty-eight folders is not a review anyone will
actually do. `make-contact-sheet.ps1` collects them into one labelled grid:

```powershell
pwsh -File make-contact-sheet.ps1
```

It writes `runs/contact-sheet.png` — every captured example at a glance, each
cell captioned with its name and a colour chip for its status. With no
arguments it reads *every* run folder and keeps the newest capture of each
example, which is what you want when evidence is spread across a full run plus
later `-RetryFrom` passes. Narrow it with `-RunId final, retry1`, or set
`-Columns` / `-CellWidth` to change the shape.

The header states what fraction is shown, because the sheet covers only the
examples that open a window — 48 of 62. It is a visual index for a human, not
the result: `report.txt` remains the record. Its real value is catching what a
green status cannot, like a video surface that renders black.

## Fixing things: the regression loop

A full pass takes around 40 minutes, and nearly all of it re-proves examples
that already work. When you are fixing something, don't pay that twice:

```powershell
pwsh -File run-references.ps1 -SkipBuild -RetryFrom final -RunId retry1
```

That runs only the examples that had not *settled* in run `final` — anything
not already `PASS`, `EXCLUDED`, `SKIPPED-HARDWARE` or `HARDWARE-OK`. The report
says plainly that it was a partial run and which run the rest came from, so a
narrowed pass can never be mistaken for a full one.

Each example writes its own `result.json` the moment it finishes, so an
interrupted run is not wasted: `-RetryFrom` rebuilds the baseline from those
per-example files when the rolled-up `results.json` was never written.

For a single shippable evidence bundle, do one clean unnarrowed run at the end.

## Evidence layout

```text
runs/
  contact-sheet.png               every screenshot in one labelled grid
  _stage/                         staged binaries + native DLLs (shared between runs)
  <run-id>/
    inventory.json                the counted set, with category, LOC, mode, documented flag
    build.json                    per-example artifacts, compile time, any compiler errors
    binaries.csv                  SHA-256 of each executable as run, with size and timestamp
    results.json                  one record per example
    report.txt                    the readable summary
    examples/<name>/
      stdout.txt  stderr.txt      captured output
      window.png                  the photographed frame, when there was a window
      result.json                 that example's record
```

`runs/` is gitignored. It is evidence to collect off the machine that ran the
assessment, not source to commit.

## Options

| Flag | Default | Purpose |
| --- | --- | --- |
| `-Frames <n>` | 600 | frames to render before a windowed example exits. Below roughly 400 the app can exit before the settle and no screenshot is taken |
| `-SettleSec <n>` | 4 | seconds after the window appears before photographing it, so asset loading is done |
| `-TimeoutSec <n>` | 120 | per-example wall-clock limit |
| `-Only <pattern>` | `*` | wildcard on example name, e.g. `-Only 'scene_document_*'` |
| `-RetryFrom <run-id>` | — | run only what did not settle in that run — see below |
| `-IncludeHardware` | off | also assess the camera/microphone example instead of skipping it |
| `-SkipBuild` | off | reuse the existing `runs/_stage/` |
| `-SkipRun` | off | inventory and build only |
| `-RunId <id>` | timestamp | name the evidence folder |
