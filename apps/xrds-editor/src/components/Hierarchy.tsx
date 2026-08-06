import { useState, useEffect, useRef, useMemo } from "react";
import type { EditorSnapshot, EditorCommand, HierarchyNode, NodeBindingSummary, NodeWatcherSummary } from "../types/bridge";
import { KIND_ICON } from "../types/bridge";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
  /** Opens a named Track in the Sequencer workspace — used by the per-node
   * Triggers rows below, which are the only place this tree references a
   * Track. Creating/listing Tracks lives in SequencerListPanel, not here. */
  onOpenTrack: (name: string) => void;
}

function groupByNodeId<T extends { node_id: number }>(items: T[]): Map<number, T[]> {
  const map = new Map<number, T[]>();
  for (const item of items) {
    const list = map.get(item.node_id) ?? [];
    list.push(item);
    map.set(item.node_id, list);
  }
  return map;
}

interface CtxMenu { id: number; x: number; y: number; }

export function Hierarchy({ snapshot, send, onOpenTrack }: Props) {
  const { hierarchy, selection } = snapshot;

  // Expand/collapse state — all nodes start expanded
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  // Separate from `expanded` (which tracks real child nodes) — the
  // synthetic "Triggers" pseudo-row's own expand state, keyed by owning
  // node id. Two more nested sets (Bindings/Watchers sub-rows) below that.
  const [expandedTriggers, setExpandedTriggers] = useState<Set<number>>(new Set());
  const [expandedBindingsFor, setExpandedBindingsFor] = useState<Set<number>>(new Set());
  const [expandedWatchersFor, setExpandedWatchersFor] = useState<Set<number>>(new Set());
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [renameVal, setRenameVal] = useState("");
  const [ctxMenu, setCtxMenu] = useState<CtxMenu | null>(null);
  const [draggingId, setDraggingId] = useState<number | null>(null);
  const [dragOverId, setDragOverId] = useState<number | "root" | null>(null);
  const renameRef = useRef<HTMLInputElement>(null);

  const bindingsByNode = useMemo(() => groupByNodeId(snapshot.all_node_bindings), [snapshot.all_node_bindings]);
  const watchersByNode = useMemo(() => groupByNodeId(snapshot.all_node_watchers), [snapshot.all_node_watchers]);

  function toggleIn(set: Set<number>, id: number, setter: (s: Set<number>) => void) {
    const next = new Set(set);
    next.has(id) ? next.delete(id) : next.add(id);
    setter(next);
  }

  // Auto-expand all on first load / hierarchy change
  useEffect(() => {
    const ids = new Set<number>();
    function collect(nodes: HierarchyNode[]) {
      nodes.forEach(n => { if (n.children.length > 0) { ids.add(n.id); collect(n.children); } });
    }
    collect(hierarchy);
    setExpanded(ids);
  }, [hierarchy.length]);

  // Close context menu on outside click
  useEffect(() => {
    if (!ctxMenu) return;
    const close = () => setCtxMenu(null);
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [ctxMenu]);

  // Focus rename input when it appears
  useEffect(() => {
    if (renamingId !== null) renameRef.current?.select();
  }, [renamingId]);

  // ── Handlers ─────────────────────────────────────────────────────────────

  function handleClick(id: number, e: React.MouseEvent) {
    if (renamingId !== null) return;
    e.ctrlKey || e.metaKey
      ? send({ type: "MultiSelectNode", payload: { id, extend: true } })
      : send({ type: "SelectNode", payload: { id } });
  }

  function handleDoubleClick(id: number, currentName: string) {
    setRenamingId(id);
    setRenameVal(currentName);
  }

  function commitRename() {
    if (renamingId !== null && renameVal.trim()) {
      send({ type: "RenameNode", payload: { id: renamingId, name: renameVal.trim() } });
    }
    setRenamingId(null);
  }

  function handleRenameKey(e: React.KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Enter")  { commitRename(); }
    if (e.key === "Escape") { setRenamingId(null); }
  }

  function handleCtx(id: number, e: React.MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    send({ type: "SelectNode", payload: { id } });
    setCtxMenu({ id, x: e.clientX, y: e.clientY });
  }

  function ctxAction(fn: () => void) {
    return (e: React.MouseEvent) => { e.stopPropagation(); setCtxMenu(null); fn(); };
  }

  function handleEmptyClick(e: React.MouseEvent) {
    if ((e.target as HTMLElement).closest(".tree-node")) return;
    send({ type: "DeselectAll" });
  }

  // ── Drag-and-drop ─────────────────────────────────────────────────────────

  function onDragStart(id: number, e: React.DragEvent) {
    setDraggingId(id);
    e.dataTransfer.effectAllowed = "move";
  }

  function onDragOver(targetId: number | "root", e: React.DragEvent) {
    e.preventDefault(); e.stopPropagation();
    e.dataTransfer.dropEffect = "move";
    setDragOverId(targetId);
  }

  function onDragLeave() { setDragOverId(null); }

  function onDrop(newParentId: number | null, e: React.DragEvent) {
    e.preventDefault(); e.stopPropagation();
    if (draggingId !== null && draggingId !== newParentId) {
      send({ type: "ReparentNode", payload: { id: draggingId, new_parent_id: newParentId, index: 0 } });
    }
    setDraggingId(null); setDragOverId(null);
  }

  function onDragEnd() { setDraggingId(null); setDragOverId(null); }

  // ── Render ────────────────────────────────────────────────────────────────

  function renderNode(node: HierarchyNode, depth: number): React.ReactNode {
    const isSelected  = selection.includes(node.id);
    const isExpanded  = expanded.has(node.id);
    const hasChildren = node.children.length > 0;
    const isRenaming  = renamingId === node.id;
    const isDragOver  = dragOverId === node.id;

    return (
      <div key={node.id}>
        <div
          className={`tree-node${isSelected ? " selected" : ""}${isDragOver ? " drag-over" : ""}`}
          style={{ paddingLeft: 4 + depth * 16 }}
          onClick={e => handleClick(node.id, e)}
          onDoubleClick={() => handleDoubleClick(node.id, node.name)}
          onContextMenu={e => handleCtx(node.id, e)}
          draggable={!isRenaming}
          onDragStart={e => onDragStart(node.id, e)}
          onDragOver={e => onDragOver(node.id, e)}
          onDragLeave={onDragLeave}
          onDrop={e => onDrop(node.id, e)}
          onDragEnd={onDragEnd}
        >
          {/* Expand/collapse arrow */}
          <span className="tree-expand"
                onClick={e => { e.stopPropagation(); setExpanded(s => { const n = new Set(s); isExpanded ? n.delete(node.id) : n.add(node.id); return n; }); }}>
            {hasChildren ? (isExpanded ? "▾" : "▸") : " "}
          </span>

          {/* Icon */}
          <span className="icon">{KIND_ICON[node.kind] ?? "○"}</span>

          {/* Name or rename input */}
          {isRenaming ? (
            <input
              ref={renameRef}
              className="tree-rename"
              value={renameVal}
              onChange={e => setRenameVal(e.target.value)}
              onBlur={commitRename}
              onKeyDown={handleRenameKey}
              onClick={e => e.stopPropagation()}
            />
          ) : (
            <span className={node.visible ? "" : "hidden"}>{node.name}</span>
          )}

          {/* Kind tag (hide when renaming) */}
          {!isRenaming && <span className="kind">{node.kind}</span>}
        </div>

        {/* Triggers pseudo-row — Bindings/Watchers authored on this node
         * (Phase D's Hierarchy grouping; see
         * docs/xrds-sequencer-v2-implementation-plan.md). Kept as two
         * separate sub-rows rather than one merged list per the design
         * assessment doc's decision — a Watcher only *fires* a Custom
         * name some other binding may listen for, it never names a
         * runnable itself, so conflating the two would obscure that. */}
        {renderTriggersRow(node, depth + 1)}

        {/* Children */}
        {hasChildren && isExpanded && node.children.map(c => renderNode(c, depth + 1))}
      </div>
    );
  }

  function renderTriggersRow(node: HierarchyNode, depth: number): React.ReactNode {
    const bindings = bindingsByNode.get(node.id) ?? [];
    const watchers = watchersByNode.get(node.id) ?? [];
    if (bindings.length === 0 && watchers.length === 0) return null;

    const triggersOpen = expandedTriggers.has(node.id);
    const pad = 4 + depth * 16;

    return (
      <div key={`triggers-${node.id}`}>
        <div className="tree-node" style={{ paddingLeft: pad }}
          onClick={() => toggleIn(expandedTriggers, node.id, setExpandedTriggers)}>
          <span className="tree-expand">{triggersOpen ? "▾" : "▸"}</span>
          <span className="icon">⚡</span>
          <span>Triggers</span>
          <span className="kind">{bindings.length + watchers.length}</span>
        </div>
        {triggersOpen && (
          <>
            {renderSummaryGroup("Bindings", bindings.length, depth + 1, node.id, expandedBindingsFor, setExpandedBindingsFor,
              bindings.map(b => renderBindingLine(b, depth + 2)))}
            {renderSummaryGroup("Watchers", watchers.length, depth + 1, node.id, expandedWatchersFor, setExpandedWatchersFor,
              watchers.map(w => renderWatcherLine(w, depth + 2)))}
          </>
        )}
      </div>
    );
  }

  function renderSummaryGroup(
    label: string, count: number, depth: number, nodeId: number,
    openSet: Set<number>, setOpenSet: (s: Set<number>) => void, children: React.ReactNode,
  ): React.ReactNode {
    if (count === 0) return null;
    const open = openSet.has(nodeId);
    return (
      <div>
        <div className="tree-node" style={{ paddingLeft: 4 + depth * 16 }}
          onClick={() => toggleIn(openSet, nodeId, setOpenSet)}>
          <span className="tree-expand">{open ? "▾" : "▸"}</span>
          <span className="text-overlay0 text-[10px]">{label}</span>
          <span className="kind">{count}</span>
        </div>
        {open && children}
      </div>
    );
  }

  function renderBindingLine(summary: NodeBindingSummary, depth: number): React.ReactNode {
    const b = summary.binding;
    const kindLabel = b.trigger.kind === "Custom" ? `Custom(${b.trigger.data})` : b.trigger.kind;
    return (
      <div key={`b-${summary.node_id}-${summary.binding_index}`} className="tree-node"
        style={{ paddingLeft: 4 + depth * 16, opacity: b.disabled ? 0.45 : 1 }}
        onClick={e => { e.stopPropagation(); if (b.track) onOpenTrack(b.track); else send({ type: "SelectNode", payload: { id: summary.node_id } }); }}
        title={b.track ? `Edit "${b.track}"` : "Select this node to edit its inline sequence"}
      >
        <span className="tree-expand"> </span>
        <span className="icon">▶</span>
        <span>{kindLabel}</span>
        <span className="kind">{b.track ?? "inline"}</span>
      </div>
    );
  }

  function renderWatcherLine(summary: NodeWatcherSummary, depth: number): React.ReactNode {
    const w = summary.watcher;
    return (
      <div key={`w-${summary.node_id}-${summary.watcher_index}`} className="tree-node"
        style={{ paddingLeft: 4 + depth * 16, opacity: w.disabled ? 0.45 : 1 }}
        onClick={e => { e.stopPropagation(); send({ type: "SelectNode", payload: { id: summary.node_id } }); }}
        title="Select this node to edit this watcher"
      >
        <span className="tree-expand"> </span>
        <span className="icon">👁</span>
        <span>{w.observable.type} {w.crossing}</span>
        <span className="kind">→ {w.fires}</span>
      </div>
    );
  }

  return (
    <div className="hierarchy">
      <div className="panel-header">Hierarchy</div>
      <div className="tree" onClick={handleEmptyClick}>
        {hierarchy.map(node => renderNode(node, 0))}

        {/* Drop-to-root zone at the bottom */}
        <div
          className={`tree-drop-root${dragOverId === "root" ? " drag-over" : ""}`}
          onDragOver={e => onDragOver("root", e)}
          onDragLeave={onDragLeave}
          onDrop={e => onDrop(null, e)}
        />
      </div>

      {/* Context menu */}
      {ctxMenu && (
        <div className="ctx-menu" style={{ left: ctxMenu.x, top: ctxMenu.y }}
             onMouseDown={e => e.stopPropagation()}>
          <div className="ctx-item" onClick={ctxAction(() => {
            const node = findNode(hierarchy, ctxMenu.id);
            if (node) handleDoubleClick(node.id, node.name);
          })}>Rename</div>
          <div className="ctx-item" onClick={ctxAction(() =>
            send({ type: "DuplicateNode", payload: { id: ctxMenu.id } }))}>Duplicate</div>
          <div className="ctx-sep" />
          <div className="ctx-item danger" onClick={ctxAction(() =>
            send({ type: "DeleteNode", payload: { id: ctxMenu.id } }))}>Delete</div>
        </div>
      )}
    </div>
  );
}

function findNode(nodes: HierarchyNode[], id: number): HierarchyNode | null {
  for (const n of nodes) {
    if (n.id === id) return n;
    const found = findNode(n.children, id);
    if (found) return found;
  }
  return null;
}
