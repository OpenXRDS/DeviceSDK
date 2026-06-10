import { useState, useEffect, useRef, useCallback } from "react";
import type { EditorSnapshot, EditorCommand, HierarchyNode } from "../types/bridge";
import { KIND_ICON } from "../types/bridge";

interface Props {
  snapshot: EditorSnapshot;
  send: (cmd: EditorCommand) => void;
}

interface CtxMenu { id: number; x: number; y: number; }

export function Hierarchy({ snapshot, send }: Props) {
  const { hierarchy, selection } = snapshot;

  // Expand/collapse state — all nodes start expanded
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [renameVal, setRenameVal] = useState("");
  const [ctxMenu, setCtxMenu] = useState<CtxMenu | null>(null);
  const [draggingId, setDraggingId] = useState<number | null>(null);
  const [dragOverId, setDragOverId] = useState<number | "root" | null>(null);
  const renameRef = useRef<HTMLInputElement>(null);

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

        {/* Children */}
        {hasChildren && isExpanded && node.children.map(c => renderNode(c, depth + 1))}
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
