import { createStore, type StoreApi } from "zustand/vanilla";
import type { Host } from "./host";
import type { PatchBatch, Workspace } from "./protocol";
import { applyBatch, PatchError } from "./reducer";

const RETRY_MS = 500;

export interface WorkspaceState {
  ready: boolean;
  rev: number;
  workspace: Workspace;
}

export interface WorkspaceStore {
  store: StoreApi<WorkspaceState>;
  /** Subscribes to patches, then loads the snapshot. */
  start(): Promise<void>;
  stop(): void;
}

/**
 * One store per window. It applies batches in rev order; a gap or a patch that does not fit
 * reloads the snapshot.
 */
export function createWorkspaceStore(host: Host): WorkspaceStore {
  const store = createStore<WorkspaceState>(() => ({
    ready: false,
    rev: 0,
    workspace: { windows: [] },
  }));
  let unlisten: (() => void) | undefined;
  let pending: PatchBatch[] | undefined = [];
  let resyncing: Promise<void> | undefined;

  const resync = (): Promise<void> => {
    resyncing ??= host
      .snapshot()
      .then((snap) => {
        const buffered = pending ?? [];
        pending = undefined;
        store.setState({ ready: true, rev: snap.rev, workspace: snap.workspace });
        for (const b of buffered.sort((a, b) => a.rev - b.rev)) receive(b);
      })
      .catch((e: unknown) => {
        // Keep buffering and try again; the core may be busy or restarting.
        console.error("terminescu: snapshot failed, retrying", e);
        setTimeout(() => void resync(), RETRY_MS);
      })
      .finally(() => {
        resyncing = undefined;
      });
    return resyncing;
  };

  const receive = (batch: PatchBatch) => {
    if (pending) {
      pending.push(batch);
      return;
    }
    const { rev, workspace } = store.getState();
    if (batch.rev <= rev) return;
    if (batch.rev !== rev + 1) {
      pending = [batch];
      void resync();
      return;
    }
    try {
      store.setState({ rev: batch.rev, workspace: applyBatch(workspace, batch) });
    } catch (e) {
      if (!(e instanceof PatchError)) throw e;
      pending = [];
      void resync();
    }
  };

  return {
    store,
    async start() {
      unlisten = await host.onPatches(receive);
      await resync();
    },
    stop() {
      unlisten?.();
      unlisten = undefined;
    },
  };
}
