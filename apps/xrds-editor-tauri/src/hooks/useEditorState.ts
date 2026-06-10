import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { defaultSnapshot, type EditorSnapshot } from "../types/bridge";

export function useEditorState(): EditorSnapshot {
  const [snapshot, setSnapshot] = useState<EditorSnapshot>(defaultSnapshot);

  useEffect(() => {
    const unlisten = listen<EditorSnapshot>("editor_state", (event) => {
      setSnapshot(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  return snapshot;
}
