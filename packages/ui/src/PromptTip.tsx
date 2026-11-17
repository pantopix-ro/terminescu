import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { BLINK_MS, type TipPart } from "./prompt-tip";

const reduced = () => typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * A prompt's body for the tooltip. Each slot holds a fixed width, so `{{name}}` and its value
 * fill the same box and nothing moves when they swap. Reduced motion shows the values, still.
 */
export function PromptTip({ parts }: { parts: TipPart[] }) {
  const still = reduced();
  const [phase, setPhase] = useState(still);
  const live = !still && parts.some((p) => "raw" in p);
  useEffect(() => {
    if (!live) return;
    const t = setInterval(() => setPhase((p) => !p), BLINK_MS);
    return () => clearInterval(t);
  }, [live]);
  const box = useRef<HTMLDivElement>(null);
  const [clipped, setClipped] = useState(false);
  useLayoutEffect(() => {
    const el = box.current;
    if (el) setClipped(el.scrollHeight > el.clientHeight + 1);
  }, [parts]);
  return (
    <div ref={box} className={`pt-body${clipped ? " clipped" : ""}`} data-phase={phase ? "value" : "name"}>
      {parts.map((p, i) =>
        "raw" in p ? (
          <span key={i} className="pt-slot" style={{ width: `${p.width}ch` }} data-slot={p.raw}>
            {phase ? p.shown : p.raw}
          </span>
        ) : (
          <span key={i}>{p.text}</span>
        ),
      )}
    </div>
  );
}
