import * as RadixSelect from "@radix-ui/react-select";
import { useEffect, useRef } from "react";
import {
  acquireViewportHoleSuppression,
  releaseViewportHoleSuppression,
} from "../../lib/viewportHole";

/** Shared, Tailwind-styled wrapper around Radix Select — the "try Radix UI"
 * pilot's replacement for a raw `<select>`. Same `onValueChange` → `send(...)`
 * data flow every caller already had with `onChange`; only the presentation
 * layer changed. See docs/done/xrds-trigger-action-editor-plan.md's
 * frontend follow-up notes. */
export interface SelectOption {
  value: string;
  label: string;
  /** Shown, not filtered out, but not selectable — pair with `hint` so the
   * reason is visible right on the option instead of only in a list that's
   * mysteriously shorter than expected. */
  disabled?: boolean;
  /** Short trailing note rendered dimmed next to the label, e.g. "needs
   * Grabbable checked" — the point of showing a disabled option at all. */
  hint?: string;
}

interface Props {
  value: string;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  disabled?: boolean;
  className?: string;
}

export function Select({ value, onValueChange, options, placeholder, disabled, className }: Props) {
  // Close the Bevy viewport hole while the dropdown is open.
  //
  // `SetWindowRgn` *clips* the WebView rather than layering it, so any part of the
  // list that extends over the 3D viewport is cut away instead of drawn on top —
  // which is what happens to a long trigger-candidate list. Handled here, once,
  // rather than at each of the ~20 call sites: a picker that forgets would be
  // silently truncated only when the list happens to be long enough to reach the
  // viewport, which is the kind of bug that survives review.
  //
  // Released on unmount too, not only on close: a component unmounted while its
  // dropdown is open (a reimport swapping the Inspector out, say) would otherwise
  // leave the hole shut for good.
  const isOpen = useRef(false);
  useEffect(() => () => {
    if (isOpen.current) {
      isOpen.current = false;
      releaseViewportHoleSuppression();
    }
  }, []);

  const handleOpenChange = (open: boolean) => {
    // Guarded against a repeat in the same state, so the refcount cannot drift if
    // Radix ever fires onOpenChange twice for one transition.
    if (open === isOpen.current) return;
    isOpen.current = open;
    if (open) acquireViewportHoleSuppression();
    else releaseViewportHoleSuppression();
  };

  return (
    <RadixSelect.Root
      value={value}
      onValueChange={onValueChange}
      disabled={disabled}
      onOpenChange={handleOpenChange}
    >
      <RadixSelect.Trigger
        className={`inline-flex items-center justify-between gap-1 rounded border border-surface1
          bg-mantle px-1.5 py-0.5 text-[11px] text-text data-[disabled]:opacity-40
          data-[disabled]:cursor-default cursor-pointer ${className ?? ""}`}
      >
        <RadixSelect.Value placeholder={placeholder} />
        <RadixSelect.Icon className="text-overlay0">▾</RadixSelect.Icon>
      </RadixSelect.Trigger>
      <RadixSelect.Portal>
        <RadixSelect.Content
          position="popper"
          sideOffset={4}
          className="z-[1000] overflow-hidden rounded border border-surface1 bg-mantle
            text-[11px] text-text shadow-lg"
        >
          <RadixSelect.Viewport className="p-1">
            {options.map(opt => (
              <RadixSelect.Item
                key={opt.value}
                value={opt.value}
                disabled={opt.disabled}
                className="flex items-center justify-between gap-3 rounded px-2 py-1 outline-none cursor-pointer
                  data-[highlighted]:bg-surface0 data-[highlighted]:text-text
                  data-[disabled]:opacity-50 data-[disabled]:cursor-not-allowed
                  data-[disabled]:data-[highlighted]:bg-transparent"
              >
                <RadixSelect.ItemText>{opt.label}</RadixSelect.ItemText>
                {opt.hint && <span className="text-[11px] text-overlay0 whitespace-nowrap">{opt.hint}</span>}
              </RadixSelect.Item>
            ))}
          </RadixSelect.Viewport>
        </RadixSelect.Content>
      </RadixSelect.Portal>
    </RadixSelect.Root>
  );
}
