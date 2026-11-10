// What plugins show on a row or a tab (add-plugin-architecture D7). The list comes composed;
// this component knows no plugin.

import type { RenderItem } from "@terminescu/plugin-api";

export function PluginItems({ items }: { items: readonly RenderItem[] | undefined }) {
  if (!items?.length) return null;
  return (
    <span className="pl-items">
      {items.map((i) => (
        <span
          key={`${i.plugin}.${i.slot}.${i.key}`}
          className={`pl-item pl-${i.slot} pl-${i.tone}`}
          data-plugin={i.plugin}
          data-key={i.key}
          title={i.tip}
        >
          {i.text}
        </span>
      ))}
    </span>
  );
}
