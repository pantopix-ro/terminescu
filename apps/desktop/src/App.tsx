import type { Command, Host, WorkspaceStore } from "@terminescu/host";
import { SideTabList, TabBar, TerminalPane } from "@terminescu/ui";
import type { Terminal } from "@xterm/xterm";
import { useEffect, useState } from "react";
import { useStore } from "zustand";

export interface AppProps {
  windowId: string;
  host: Host;
  workspace: WorkspaceStore;
  /** Collects each tab's terminal, for Playwright on the mock host. */
  terminals?: Map<string, Terminal>;
}

/** Keeps a selection valid: the previous id while it still exists, else the first one. */
function pick(ids: string[], previous: string | undefined): string | undefined {
  return previous && ids.includes(previous) ? previous : ids[0];
}

export function App({ windowId, host, workspace, terminals }: AppProps) {
  const ws = useStore(workspace.store, (s) => s.workspace);
  const win = ws.windows.find((w) => w.id === windowId);
  const sideTabs = win?.sideTabs ?? [];

  const [sideTabId, setSideTabId] = useState<string>();
  const activeSide = sideTabs.find((s) => s.id === pick(sideTabs.map((s) => s.id), sideTabId));
  const tabs = activeSide?.tabs ?? [];
  const [tabBySide, setTabBySide] = useState<Record<string, string>>({});
  const activeTabId = activeSide && pick(tabs.map((t) => t.id), tabBySide[activeSide.id]);

  const [error, setError] = useState<string>();
  const send = (c: Command) => host.send(c).catch((e: unknown) => setError(JSON.stringify(e)));

  // Select what a command just created: the last side tab or the last tab.
  const [follow, setFollow] = useState<"side" | "tab">();
  useEffect(() => {
    if (follow === "side" && sideTabs.length) setSideTabId(sideTabs.at(-1)!.id);
    if (follow === "tab" && activeSide && tabs.length)
      setTabBySide((m) => ({ ...m, [activeSide.id]: tabs.at(-1)!.id }));
    setFollow(undefined);
  }, [ws]);

  return (
    <div className="app">
      <aside className="app-side">
        <div className="app-traffic" data-tauri-drag-region />
        <SideTabList
          sideTabs={sideTabs}
          activeId={activeSide?.id}
          onSelect={setSideTabId}
          onNew={() => {
            setFollow("side");
            void send({ type: "CreateSideTab", windowId });
          }}
        />
      </aside>
      <main className="app-main">
        <TabBar
          tabs={tabs}
          activeId={activeTabId}
          onSelect={(id) => activeSide && setTabBySide((m) => ({ ...m, [activeSide.id]: id }))}
          onClose={(tabId) => void send({ type: "CloseTab", tabId })}
          onNew={() => {
            if (!activeSide) return;
            setFollow("tab");
            void send({ type: "CreateTab", sideTabId: activeSide.id });
          }}
        />
        <section className="app-pane">
          {/* Every tab of the window keeps its terminal; only the selected one shows (D33). */}
          {sideTabs.flatMap((side) =>
            side.tabs.map((tab) => (
              <TerminalPane
                key={tab.id}
                tabId={tab.id}
                visible={side.id === activeSide?.id && tab.id === activeTabId}
                open={(cols, rows, onOutput) => host.openPane(tab.id, cols, rows, onOutput)}
                openUrl={(url) => void host.openUrl(url).catch((e: unknown) => setError(String(e)))}
                onTerminal={terminals && ((t) => (t ? terminals.set(tab.id, t) : terminals.delete(tab.id)))}
              />
            )),
          )}
          {!activeTabId && <p className="small app-empty">No side tabs. Press + in the sidebar.</p>}
          {error && <p className="small app-error">{error}</p>}
        </section>
      </main>
    </div>
  );
}
