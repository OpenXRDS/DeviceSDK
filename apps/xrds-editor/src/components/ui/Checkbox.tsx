import * as RadixCheckbox from "@radix-ui/react-checkbox";

/** Shared, Tailwind-styled wrapper around Radix Checkbox — the "try Radix
 * UI" pilot's replacement for a raw `<input type="checkbox">`. Same
 * `onCheckedChange` → `send(...)` data flow every caller already had with
 * `onChange`. See docs/done/xrds-trigger-action-editor-plan.md's frontend
 * follow-up notes. */
interface Props {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  label?: string;
  disabled?: boolean;
  title?: string;
  className?: string;
}

export function Checkbox({ checked, onCheckedChange, label, disabled, title, className }: Props) {
  return (
    <label
      className={`inline-flex items-center gap-1 text-[10px] text-overlay0
        ${disabled ? "opacity-40" : "cursor-pointer"} ${className ?? ""}`}
      title={title}
    >
      <RadixCheckbox.Root
        checked={checked}
        onCheckedChange={v => onCheckedChange(v === true)}
        disabled={disabled}
        className="flex h-[15px] w-[15px] items-center justify-center rounded-sm
          border border-surface1 bg-mantle data-[state=checked]:bg-green
          data-[state=checked]:border-green data-[disabled]:cursor-default"
      >
        <RadixCheckbox.Indicator className="text-crust text-[11px] leading-none">
          ✓
        </RadixCheckbox.Indicator>
      </RadixCheckbox.Root>
      {label}
    </label>
  );
}
