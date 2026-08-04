import * as RadixSelect from "@radix-ui/react-select";

/** Shared, Tailwind-styled wrapper around Radix Select — the "try Radix UI"
 * pilot's replacement for a raw `<select>`. Same `onValueChange` → `send(...)`
 * data flow every caller already had with `onChange`; only the presentation
 * layer changed. See docs/done/xrds-trigger-action-editor-plan.md's
 * frontend follow-up notes. */
export interface SelectOption {
  value: string;
  label: string;
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
  return (
    <RadixSelect.Root value={value} onValueChange={onValueChange} disabled={disabled}>
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
                className="flex items-center rounded px-2 py-1 outline-none cursor-pointer
                  data-[highlighted]:bg-surface0 data-[highlighted]:text-text"
              >
                <RadixSelect.ItemText>{opt.label}</RadixSelect.ItemText>
              </RadixSelect.Item>
            ))}
          </RadixSelect.Viewport>
        </RadixSelect.Content>
      </RadixSelect.Portal>
    </RadixSelect.Root>
  );
}
