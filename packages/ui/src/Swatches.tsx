import { ACCENTS, TINTS, type Accent, type Tint } from "@terminescu/host";

const NAMES: Record<string, string> = { blue: "Blue", cyan: "Cyan", green: "Green", magenta: "Magenta", red: "Red", fg: "White" };

function swatchStyle(c: string, accent: boolean, size: object): React.CSSProperties {
  const vars: Record<string, string> = accent ? { "--sw": c === "fg" ? "var(--fg)" : `var(--${c})` } : { "--sw": `var(--tint-${c})`, "--swe": `var(--${c})` };
  return { ...size, ...vars } as React.CSSProperties;
}

type Props =
  | { kind: "accent"; value: Accent | null; onChange(v: Accent | null): void; small?: boolean }
  | { kind: "background"; value: Tint | null; onChange(v: Tint | null): void; small?: boolean };

/** Accent or background picker. Amber is never offered: it means "needs you" (D16). */
export function Swatches(props: Props) {
  const accent = props.kind === "accent";
  const options: readonly string[] = accent ? ACCENTS : TINTS;
  const size = props.small ? { width: 16, height: 16 } : {};
  const pick = (v: string | null) => (props.onChange as (v: string | null) => void)(v);
  return (
    <div className="ao-swatches" role="radiogroup" aria-label={accent ? "Accent" : "Background"}>
      <button
        type="button"
        className={`ao-sw none${props.value === null ? " on" : ""}`}
        role="radio"
        aria-checked={props.value === null}
        aria-label={accent ? "No accent" : "No background"}
        style={size}
        onClick={() => pick(null)}
      />
      {options.map((c) => (
        <button
          key={c}
          type="button"
          className={`ao-sw${props.value === c ? " on" : ""}`}
          role="radio"
          aria-checked={props.value === c}
          aria-label={accent ? NAMES[c] : `${NAMES[c]} tint`}
          style={swatchStyle(c, accent, size)}
          onClick={() => pick(c)}
        />
      ))}
    </div>
  );
}
