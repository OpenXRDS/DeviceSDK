import { useEffect, useState } from "react";
import { defaultSnapshot, type EditorSnapshot } from "../types/bridge";

declare global {
  interface Window {
    __xrds__?: {
      onEditorState?: (snap: EditorSnapshot) => void;
      dialogs?: Record<string, (result: string | null) => void>;
    };
  }
}

export function useEditorState(): EditorSnapshot {
  const [snapshot, setSnapshot] = useState<EditorSnapshot>(defaultSnapshot);

  useEffect(() => {
    window.__xrds__ ??= {};
    window.__xrds__.onEditorState = (snap: EditorSnapshot) => {
      setSnapshot(snap);
    };
    return () => { if (window.__xrds__) delete window.__xrds__.onEditorState; };
  }, []);

  return snapshot;
}
