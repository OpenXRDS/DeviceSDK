import type { EditorCommand } from "../types/bridge";

interface Props {
  send: (cmd: EditorCommand) => void;
}

/**
 * Centre viewport placeholder.
 *
 * In the wry+Bevy native rendering setup, SetWindowRgn carves a hole in the
 * WebView at this div's position, letting Bevy's DXGI swap-chain show through.
 * Mouse and keyboard events in the hole go directly to Bevy — no forwarding needed.
 * The `send` prop is kept for API compatibility but is not used here.
 */
export function ViewportCanvas({ send: _send }: Props) {
  return <div className="viewport-canvas" />;
}
