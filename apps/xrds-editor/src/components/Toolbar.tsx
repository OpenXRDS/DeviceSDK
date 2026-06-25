import { useState } from "react";
import type { EditorSnapshot, EditorCommand } from "../types/bridge";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  onSaveAs: () => void;
}

function sendStereoIpc(enabled: boolean, ipd_mm: number) {
  (window as any).ipc?.postMessage(JSON.stringify({
    type: "stereo_preview",
    enabled,
    ipd_mm,
    fov_deg: 90,
  }));
}

export function Toolbar({ snapshot, send, onSaveAs }: Props) {
  const {
    scene_name, is_dirty, undo_count, redo_count,
    gizmo_mode, camera_mode, show_grid, show_fov_overlay, is_playing, selection,
    available_cameras, active_camera_id, stereo_preview_active,
  } = snapshot;

  const [stereoIpd, setStereoIpd] = useState(63);

  return (
    <div className="toolbar">
      <span className="scene-name">{scene_name || "Untitled"}</span>
      {is_dirty && <span className="dirty" title="Unsaved changes">●</span>}
      <span className="meta">undo: {undo_count} / redo: {redo_count}</span>
      <span className="meta">{selection.length === 0 ? "nothing selected" : `${selection.length} selected`}</span>

      {/* Gizmo mode */}
      <div className="tb-group">
        {(["Translate","Rotate","Scale"] as const).map((m, i) => (
          <button key={m} className={`tb-btn${gizmo_mode === m ? " active" : ""}`}
                  title={`${m} (${["T","R","S"][i]})`}
                  onClick={() => send({ type: "SetGizmoMode", payload: { mode: m } })}>
            {["T","R","S"][i]}
          </button>
        ))}
      </div>

      <button className="tb-btn"
              title="Toggle Camera Mode (V)"
              onClick={() => send({ type: "SetCameraMode", payload: { mode: camera_mode === "Orbit" ? "Fly" : "Orbit" } })}>
        {camera_mode}
      </button>

      {/* Camera selector — only shown when there are scene cameras */}
      {available_cameras.length > 0 && (
        <select
          className="tb-select"
          title="Active viewport camera"
          value={active_camera_id ?? "editor"}
          onChange={e => {
            const v = e.target.value;
            send({ type: "SetActiveCamera", payload: { id: v === "editor" ? null : Number(v) } });
          }}
        >
          <option value="editor">Editor Camera</option>
          {available_cameras.map(c => (
            <option key={c.id} value={c.id}>{c.name}</option>
          ))}
        </select>
      )}

      <button className={`tb-btn${show_grid ? " active" : ""}`}
              title="Toggle Grid (G)"
              onClick={() => send({ type: "ToggleGrid" })}>
        Grid
      </button>

      <button className={`tb-btn${show_fov_overlay ? " active" : ""}`}
              title="Toggle FOV overlay for all Player Anchors"
              onClick={() => send({ type: "ToggleFovOverlay" })}>
        FOV
      </button>

      <button className="tb-btn" title="Frame Selected (F)"
              onClick={() => send({ type: "FrameSelected" })}>F</button>

      <button className={`tb-btn${is_playing ? " play-active" : ""}`}
              title="Play / Stop (F5)"
              style={{ marginLeft: 8 }}
              onClick={() => send({ type: "SetPlayMode", payload: { playing: !is_playing } })}>
        {is_playing ? "■ Stop" : "▶ Play"}
      </button>

      <button className={`tb-btn${stereo_preview_active ? " active" : ""}`}
              title="Stereo Preview — split viewport L/R eye"
              style={{ marginLeft: 8 }}
              onClick={() => sendStereoIpc(!stereo_preview_active, stereoIpd)}>
        L|R
      </button>
      {stereo_preview_active && (
        <label style={{ marginLeft: 4, fontSize: "0.78em", display: "flex", alignItems: "center", gap: 3 }}
               title="Inter-pupillary distance (mm) — controls L/R eye separation">
          IPD
          <input
            type="number"
            min={45} max={80} step={1}
            value={stereoIpd}
            style={{ width: 40, fontSize: "0.9em" }}
            onChange={e => {
              const val = Math.max(45, Math.min(80, Number(e.target.value)));
              setStereoIpd(val);
              sendStereoIpc(true, val);
            }}
          />
          mm
        </label>
      )}
    </div>
  );
}
