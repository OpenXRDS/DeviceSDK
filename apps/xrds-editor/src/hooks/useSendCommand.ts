import { useCallback } from "react";
import type { EditorCommand } from "../types/bridge";

export function useSendCommand() {
  return useCallback((command: EditorCommand) => {
    (window as any).ipc?.postMessage(JSON.stringify({ type: "command", command }));
  }, []);
}
