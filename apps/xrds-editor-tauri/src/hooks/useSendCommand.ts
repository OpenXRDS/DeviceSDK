import { invoke } from "@tauri-apps/api/core";
import { useCallback } from "react";
import type { EditorCommand } from "../types/bridge";

export function useSendCommand() {
  return useCallback((command: EditorCommand) => {
    invoke("send_editor_command", { command }).catch(console.error);
  }, []);
}
