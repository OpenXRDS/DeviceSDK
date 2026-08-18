import type { EditorSnapshot, EditorCommand } from "../types/bridge";
import { KIND_ICON } from "../types/bridge";
import { useResizable } from "../hooks/useResizable";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
}

export function PlayerPanel({ snapshot, send }: Props) {
  const { player_anchors, active_player_anchor_id, show_fov_overlay } = snapshot;
  // Handle sits on this panel's own top edge — drag up to grow it,
  // encroaching into Hierarchy's space above. Called before the early
  // return below so hook order stays stable across renders either way.
  const { size: height, dragging, onPointerDown } =
    useResizable({ axis: "y", initial: 130, min: 60, max: 360, invert: true });

  if (player_anchors.length === 0) return null;

  function toggle(id: number) {
    send({
      type: "SetActivePlayerAnchor",
      payload: { id: active_player_anchor_id === id ? null : id },
    });
  }

  return (
    <div className="player-panel" style={{ height }}>
      <div className={`panel-resize-handle--h${dragging ? " dragging" : ""}`}
        onPointerDown={onPointerDown} title="Drag to resize" />
      <div className="player-panel-header">
        <span>{KIND_ICON.Player} Players</span>
        <div style={{ display: "flex", gap: 4, marginLeft: "auto" }}>
          <button
            className={`player-panel-clear${show_fov_overlay ? " fov-active" : ""}`}
            title="Toggle FOV overlay for all anchors"
            style={show_fov_overlay ? { color: "var(--blue)" } : {}}
            onClick={() => send({ type: "ToggleFovOverlay" })}
          >FOV</button>
          {active_player_anchor_id !== null && (
            <button
              className="player-panel-clear"
              title="Clear active anchor (all anchors active)"
              onClick={() => send({ type: "SetActivePlayerAnchor", payload: { id: null } })}
            >✕</button>
          )}
        </div>
      </div>
      <div className="player-panel-list">
        {player_anchors.map(a => {
          const isActive = active_player_anchor_id === a.id;
          return (
            <div key={a.id} className={`player-anchor-item${isActive ? " active" : ""}`}>
              <button
                className="anchor-select-btn"
                onClick={() => toggle(a.id)}
                title={a.player_name ? `Activate: ${a.name} (Player: ${a.player_name})` : `Activate: ${a.name}`}
              >
                <span className="anchor-icon">{KIND_ICON.PlayerAnchor}</span>
                <span className="anchor-name">{a.name}</span>
                {a.player_name && (
                  <span className="anchor-parent">{a.player_name}</span>
                )}
                {isActive && <span className="anchor-badge">●</span>}
              </button>
              <button
                className="anchor-preview-btn"
                onClick={() => {
                  // Play mode: teleport pawn by activating the anchor.
                  // Edit mode: move editor camera to the anchor's authored position.
                  send({ type: "SetActivePlayerAnchor", payload: { id: a.id } });
                  if (!snapshot.is_playing) {
                    send({ type: "PreviewFromAnchor", payload: { id: a.id } });
                  }
                }}
                title={snapshot.is_playing ? "Switch pawn to this anchor" : "Move editor camera to this anchor's position"}
              >👁</button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
