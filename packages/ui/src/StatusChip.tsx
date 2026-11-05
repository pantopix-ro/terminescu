import type { Chip } from "./status";
import { chipStyle } from "./status";

/** A status label in the design system's `ao-stat` style. */
export function StatusChip({ chip, className = "" }: { chip: Chip; className?: string }) {
  return (
    <span
      className={`ao-stat${chip.set ? " manual" : ""}${chip.need ? " need" : ""} ${className}`.trim()}
      style={chipStyle(chip)}
      data-status={chip.label}
    >
      {chip.label}
    </span>
  );
}
