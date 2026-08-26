import * as React from "react";
import * as RadixSelect from "@radix-ui/react-select";
import { useEffect, useId, useRef, useState } from "react";
import { clearOccluder, trackOccluder } from "../../lib/uiOccluders";

/** Shared, Tailwind-styled wrapper around Radix Select — the "try Radix UI"
 * pilot's replacement for a raw `<select>`. Same `onValueChange` → `send(...)`
 * data flow every caller already had with `onChange`; only the presentation
 * layer changed. See docs/done/xrds-trigger-action-editor-plan.md's
 * frontend follow-up notes. */
export interface SelectOption {
  value: string;
  /** `ReactNode` so an option can carry an icon beside its text. The trigger
   *  renders the selected option's label too, so it must be renderable there. */
  label: React.ReactNode;
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
  // Punch the open dropdown's own rectangle out of the Bevy viewport hole.
  //
  // `SetWindowRgn` *clips* the WebView rather than layering it, so any part of the
  // list extending over the 3D viewport is cut away instead of drawn on top — which
  // is what happens to a long trigger-candidate list. Reporting the rectangle keeps
  // the rest of the viewport live; the earlier attempt closed the hole entirely and
  // turned the whole 3D view black while any picker was open. See `lib/uiOccluders`.
  //
  // Handled here, once, rather than at each of the ~20 call sites: a picker that
  // forgot would be truncated only when its list happened to be long enough to
  // reach the viewport, which is the kind of bug that survives review.
  const occluderId = useId();
  const contentRef = useRef<HTMLDivElement | null>(null);
  // State, not a ref: the tracking effect below has to re-run when the dropdown
  // opens, and a ref mutation alone would not re-run it.
  const [open, setOpen] = useState(false);

  // Radix positions the content *after* mount and repositions it on scroll and on
  // collision with the window edge, so one measurement at open time is not enough:
  // a stale rectangle leaves a lit patch of WebView beside a sliced dropdown. Track
  // it for as long as the list is open. rAF rather than a ResizeObserver, which
  // reports resizes but never *movement*.
  //
  // The cleanup also covers unmount-while-open — a reimport swapping the Inspector
  // out mid-dropdown would otherwise leave a permanent bite out of the viewport.
  useEffect(() => {
    if (!open) {
      clearOccluder(occluderId);
      return;
    }
    let raf = 0;
    const tick = () => {
      trackOccluder(occluderId, contentRef.current);
      raf = requestAnimationFrame(tick);
    };
    tick();
    return () => {
      cancelAnimationFrame(raf);
      clearOccluder(occluderId);
    };
  }, [open, occluderId]);

  return (
    <RadixSelect.Root
      value={value}
      onValueChange={onValueChange}
      disabled={disabled}
      open={open}
      onOpenChange={setOpen}
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
          ref={contentRef}
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
