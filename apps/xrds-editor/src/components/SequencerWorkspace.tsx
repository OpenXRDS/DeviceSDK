import { useEffect, useMemo, useRef, useState } from "react";
import type { EditorCommand, EditorSnapshot, XrdsTrackKeyDto } from "../types/bridge";
import {
  ROW_H, actionDuration, addableAssets, assetRowAspects, assetRowLabel,
  buildTriggerReverseIndex, conflictingTracks, fmtTime, keyTopPx, layoutAssetRow,
  niceStep, rulerTicks,
} from "../lib/sequencer";
import { Checkbox } from "./ui/Checkbox";
import { Select } from "./ui/Select";
import { ACTION_COLOR, SequencerInspector, summarizeAction } from "./SequencerInspector";
import type { SelectedEvent } from "./SequencerInspector";

interface Props {
  /** Which Track is open, by name. `null` shows the empty state. */
  track: string | null;
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  selected: SelectedEvent | null;
  onSelectedChange: (s: SelectedEvent | null) => void;
}

/** Row height, shared by the asset-name column and the lane column so a row's
 * label always sits on the line with its events. They are two independent flex
 * columns, not a table, so nothing else enforces it. */
const ADD_ASSET_SENTINEL = "__add_asset__";

/** Time ruler across the top of the lane column. Major ticks carry an `m:ss`
 * label; minor ticks subdivide. Tick maths lives in lib/sequencer.ts so it is
 * unit-tested. */
function Ruler({ duration }: { duration: number }) {
  const step = niceStep(duration);
  const majors = rulerTicks(duration);
  return (
    <div className="seq-ruler">
      {majors.map(t => (
        <div key={t} className="seq-ruler-major" style={{ left: `${(t / duration) * 100}%` }}>
          <span className="seq-ruler-label">{fmtTime(t)}</span>
        </div>
      ))}
      {majors.slice(0, -1).map(t => (
        <div key={`m${t}`} className="seq-ruler-minor"
          style={{ left: `${((t + step / 2) / duration) * 100}%` }} />
      ))}
    </div>
  );
}

/** Bottom-docked Sequencer: a transport header, then three columns — asset
 * rows | ruler + lanes | event inspector. See docs/xrds-track-model-plan.md.
 *
 * Rows come straight from the Track's `assets`; there is no lane *derivation*
 * any more. Mute/Solo/Lock are gone — a deliberate simplification.
 *
 * The playhead is real during preview (the editor owns the agent and reads its
 * elapsed time) but **not draggable**: seeking would need every crossed event
 * re-evaluated, which the runtime has no API for. */
export function SequencerWorkspace({
  track: trackName, snapshot, send, selected, onSelectedChange: setSelected,
}: Props) {
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState("");
  const [activeAssetIndex, setActiveAssetIndex] = useState<number | null>(null);

  const track = trackName === null
    ? null
    : snapshot.tracks.find(t => t.name === trackName) ?? null;

  const preview = snapshot.track_preview;
  const isPreviewingThis = preview !== null && track !== null && preview.name === track.name;

  // The ruler always spans at least a little, so a Track with a single event at
  // t=0 still renders a usable axis rather than dividing by zero.
  const duration = Math.max(track?.effective_duration_secs ?? 0, 1);

  // Lane pixel width, measured live: converting the dot's fixed on-screen
  // footprint into a *time* epsilon for stackKeys needs to know how many
  // pixels a second currently occupies, which depends on the container's
  // actual rendered width, not just `duration` — the same Track reflows
  // differently as the Inspector column resizes or the window changes.
  const lanesRef = useRef<HTMLDivElement>(null);
  const [lanesWidthPx, setLanesWidthPx] = useState(0);
  // Keyed on whether a Track is open, NOT `[]`: the observed element only
  // exists while one is, so a mount-once effect would attach nothing (the
  // Sequencer opens on "No Track open") and never retry when one is picked.
  const hasTrack = track !== null;
  useEffect(() => {
    const el = lanesRef.current;
    if (!el) return;
    // Measure immediately as well as on resize — ResizeObserver does fire
    // once on observe(), but reading it here means the very first paint
    // already has a real width instead of a fallback.
    setLanesWidthPx(el.getBoundingClientRect().width);
    const observer = new ResizeObserver(entries => setLanesWidthPx(entries[0].contentRect.width));
    observer.observe(el);
    return () => observer.disconnect();
  }, [hasTrack]);

  const rowLayouts = useMemo(() => {
    if (track === null) return [];
    return track.assets.map(asset => layoutAssetRow(asset.keys, lanesWidthPx, duration));
  }, [track, duration, lanesWidthPx]);

  const firesWhen = useMemo(() => {
    if (track === null) return [];
    const idx = buildTriggerReverseIndex(snapshot.all_node_bindings);
    return (idx.byTrack.get(track.name) ?? []).map(s =>
      s.binding.trigger.kind === "Custom"
        ? `Custom(${s.binding.trigger.data})`
        : s.binding.trigger.kind,
    );
  }, [track, snapshot.all_node_bindings]);

  // Diagnostics naming this Track. Exact-quoted match on the form Rust's
  // `Debug` for `String` produces, so similarly-named Tracks cannot collide.
  const diagnostics = useMemo(() => {
    if (track === null) return [];
    const needle = `Track ${JSON.stringify(track.name)}`;
    return snapshot.track_diagnostics.filter(d => d.detail.includes(needle));
  }, [track, snapshot.track_diagnostics]);
  const errorCount = diagnostics.filter(d => d.severity === "error").length;

  const conflicts = track === null ? [] : conflictingTracks(track.name, snapshot.tracks);
  const eventCount = track?.assets.reduce((n, a) => n + a.keys.length, 0) ?? 0;

  function commitRename() {
    if (track !== null) {
      const newName = renameValue.trim();
      if (newName && newName !== track.name) {
        send({ type: "RenameTrack", payload: { old_name: track.name, new_name: newName } });
      }
    }
    setRenaming(false);
  }

  return (
    <div className="seq-ws">
      {/* ── Header / transport ──────────────────────────────────────────── */}
      <div className="seq-ws-header">
        <span className="text-[12px] font-semibold text-bright">Sequencer</span>

        {track !== null && (
          <>
            <span className="text-surface1">—</span>
            {renaming ? (
              <input autoFocus value={renameValue}
                className="text-[11.5px] text-bright bg-well rounded px-2 py-0.5 border border-surface0 focus:outline focus:outline-1 focus:outline-blue"
                onKeyDown={e => {
                  e.stopPropagation();
                  if (e.key === "Enter") commitRename();
                  if (e.key === "Escape") setRenaming(false);
                }}
                onChange={e => setRenameValue(e.target.value)}
                onBlur={commitRename} />
            ) : (
              <span className="seq-target-chip cursor-text" title="Double-click to rename"
                onDoubleClick={() => { setRenameValue(track.name); setRenaming(true); }}>
                <span className="seq-dot" style={{ background: "var(--blue)" }} />
                {track.name}
              </span>
            )}

            <span className="seq-ws-divider" />

            {/* Preview transport — independent of the toolbar's sim Play. Not
                gated on isPreviewingThis: it also has to work once a preview
                has already finished on its own, which is exactly when an
                author reaches for it. */}
            <button className="seq-transport-btn"
              title="Restart from 0:00, restoring every asset first"
              onClick={() => send({ type: "PreviewPlayTrack", payload: { name: track.name } })}>
              ⏮
            </button>
            <button className={`seq-transport-btn${isPreviewingThis && preview!.playing ? " active" : ""}`}
              title={isPreviewingThis && preview!.playing
                ? "Pause the preview"
                : "Preview this Track in the editor. Separate from the simulation's Play."}
              onClick={() => {
                if (isPreviewingThis) {
                  send({ type: "PreviewPauseTrack", payload: { paused: preview!.playing } });
                } else {
                  send({ type: "PreviewPlayTrack", payload: { name: track.name } });
                }
              }}>
              {isPreviewingThis && preview!.playing ? "⏸" : "▶"}
            </button>
            <button className="seq-transport-btn"
              title="Stop the preview and put every asset back where the document says it is"
              disabled={!isPreviewingThis}
              onClick={() => send({ type: "PreviewStopTrack" })}>
              ■
            </button>

            <span className="seq-timecode">
              {fmtTime(isPreviewingThis ? preview!.elapsed_secs : 0)}
              <span className="seq-timecode-sub">/ {fmtTime(duration)}</span>
            </span>

            <span className="seq-ws-divider" />

            <label className="text-[10.5px] text-overlay0">Duration</label>
            <input type="number" step={0.1} min={0}
              value={track.duration_secs ?? ""}
              placeholder="auto"
              title="Blank means: however long the events actually span, including a trailing interpolation."
              className="w-[64px] text-[11px] text-bright bg-well rounded px-1.5 py-0.5 border border-surface0 focus:outline focus:outline-1 focus:outline-blue font-mono"
              onKeyDown={e => e.stopPropagation()}
              onChange={e => send({
                type: "SetTrackDuration",
                payload: { name: track.name, duration_secs: e.target.value === "" ? null : +e.target.value },
              })} />
            <Checkbox label="Loop" checked={track.looping}
              title={conflicts.length > 0
                ? "Careful: a looping Track never releases its assets, so anything sharing one can never run."
                : "Repeat forever once fired."}
              onCheckedChange={v => send({
                type: "SetTrackLooping",
                payload: { name: track.name, looping: v },
              })} />
          </>
        )}

        <span className="flex-1" />

        {/* A refused Track is otherwise a silent no-op, so surface it. */}
        {snapshot.track_conflict !== null && (
          <span className="text-[10px] text-peach"
            title={`Blocked by a Track already driving: ${snapshot.track_conflict.contended.join(", ")}`}>
            ⇄ {snapshot.track_conflict.blocked_track} was refused
          </span>
        )}
      </div>

      {/* ── Body ────────────────────────────────────────────────────────── */}
      {track === null ? (
        <div className="seq-ws-empty">
          <div className="text-[12.5px] text-subtext0">No Track open</div>
          <div className="text-[11px] text-overlay0">
            Pick one from the <strong>Tracks</strong> list on the left, or open the Track a
            node's trigger fires from the Inspector.
          </div>
        </div>
      ) : (
        <div className="seq-ws-body">
          {/* Asset rows */}
          <div className="seq-ws-tracks">
            <div className="seq-ws-col-head">
              <span className="seq-caption">ASSETS</span>
              <span className="flex-1" />
              <Select
                value={ADD_ASSET_SENTINEL}
                onValueChange={v => {
                  if (v === ADD_ASSET_SENTINEL) return;
                  send({ type: "AddTrackAsset", payload: { track: track.name, node_id: +v } });
                }}
                options={[
                  { value: ADD_ASSET_SENTINEL, label: "+ Asset…" },
                  ...addableAssets(track, snapshot).map(n => ({
                    value: String(n.id),
                    label: n.name,
                  })),
                ]}
              />
            </div>
            <div className="seq-ws-track-list">
              {track.assets.map((asset, i) => {
                const label = assetRowLabel(asset);
                const aspects = assetRowAspects(asset);
                const isActive = (selected?.assetIndex ?? activeAssetIndex) === i;
                return (
                  <div key={i}
                    className={`seq-ws-track-row${isActive ? " active" : ""}`}
                    style={{ height: rowLayouts[i]?.height ?? ROW_H }}
                    onClick={() => setActiveAssetIndex(i)}>
                    <span className="seq-dot"
                      style={{ background: ACTION_COLOR[asset.keys[0]?.action.kind] ?? "var(--surface1)" }} />
                    <div className="flex flex-col min-w-0 gap-px">
                      <span className="text-[11.5px] truncate" title={label.title}>
                        {label.title}
                        {aspects.length > 0 && (
                          <span className="text-overlay0"> · {aspects.join(", ")}</span>
                        )}
                      </span>
                      <span className="text-[9px] text-overlay0 font-mono truncate">{label.sub}</span>
                    </div>
                    <span className="flex-1" />
                    <button className="seq-list-row-del"
                      title="Remove this asset row and all its events"
                      onClick={e => {
                        e.stopPropagation();
                        send({ type: "RemoveTrackAsset", payload: { track: track.name, asset_index: i } });
                        setSelected(null);
                      }}>✕</button>
                  </div>
                );
              })}
              {track.assets.length === 0 && (
                <div className="seq-list-empty">
                  No assets yet — add one above, then place events on its row.
                </div>
              )}
            </div>
          </div>

          {/* Ruler + lanes */}
          <div className="seq-ws-lanes">
            <Ruler duration={duration} />
            <div className="seq-ws-lane-list" ref={lanesRef}>
              {track.assets.map((asset, assetIndex) => {
                const layout = rowLayouts[assetIndex] ?? { height: ROW_H, lanes: [], count: 1 };
                return (
                  <div key={assetIndex} className="seq-ws-lane" style={{ height: layout.height }}>
                    {asset.keys.map((key, keyIndex) => {
                      const colour = ACTION_COLOR[key.action.kind] ?? "var(--surface1)";
                      const dur = actionDuration(key.action);
                      const isSel = selected?.assetIndex === assetIndex
                        && selected?.keyIndex === keyIndex;
                      const lane = layout.lanes[keyIndex] ?? 0;
                      return (
                        <button key={keyIndex}
                          className={`seq-key${isSel ? " selected" : ""}${dur > 0 ? " seq-key-bar" : ""}`}
                          style={{
                            left: `${(key.at_secs / duration) * 100}%`,
                            top: `${keyTopPx(lane, layout.count, layout.height)}px`,
                            width: dur > 0 ? `${(dur / duration) * 100}%` : undefined,
                            ["--k" as string]: colour,
                          }}
                          title={`t=${key.at_secs.toFixed(2)}s · ${summarizeAction(key.action)}`}
                          onClick={() => setSelected({ assetIndex, keyIndex })}
                        />
                      );
                    })}
                  </div>
                );
              })}
              {/* Live playhead — read-only. */}
              {isPreviewingThis && (
                <div className="seq-playhead"
                  style={{ left: `${Math.min(100, (preview!.elapsed_secs / duration) * 100)}%` }} />
              )}
            </div>
          </div>

          {/* Event inspector */}
          <SequencerInspector
            track={track}
            snapshot={snapshot}
            send={send}
            selected={selected}
            onSelectedChange={setSelected}
            activeAssetIndex={activeAssetIndex}
            firesWhen={firesWhen}
          />
        </div>
      )}

      {/* ── Status bar ──────────────────────────────────────────────────── */}
      <div className="seq-ws-status">
        <span className={errorCount > 0 ? "text-red" : "text-subtext0"}>
          {errorCount > 0 ? `${errorCount} error${errorCount === 1 ? "" : "s"}` : "Ready"}
        </span>
        <span className="text-surface1">|</span>
        <span>{selected === null ? "nothing selected" : "1 event selected"}</span>
        <span className="text-surface1">|</span>
        <span>
          {track?.assets.length ?? 0} asset{(track?.assets.length ?? 0) === 1 ? "" : "s"} ·{" "}
          {eventCount} event{eventCount === 1 ? "" : "s"}
        </span>
        {conflicts.length > 0 && (
          <>
            <span className="text-surface1">|</span>
            <span className="text-peach"
              title="Two Tracks driving one asset cannot run at the same time.">
              shares assets with {conflicts.join(", ")}
            </span>
          </>
        )}
        <span className="flex-1" />
        {diagnostics.length > 0 ? (
          <span className="flex items-center gap-3 overflow-hidden">
            {diagnostics.slice(0, 3).map((d, i) => (
              <span key={i}
                className={d.severity === "error" ? "text-red" : d.severity === "warning" ? "text-yellow" : ""}
                title={d.detail}>⚠ {d.title}</span>
            ))}
            {diagnostics.length > 3 && (
              <span className="text-overlay0">+{diagnostics.length - 3} more</span>
            )}
          </span>
        ) : (
          <span className="text-green">no validation errors</span>
        )}
      </div>
    </div>
  );
}
