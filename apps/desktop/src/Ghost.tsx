import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useState } from "react";

/** Task 8.1: the core's ghost window, showing the dragged tab's title. */
export function Ghost() {
  const [title, setTitle] = useState("");
  useEffect(() => {
    // The first title may come before this listens; ask for it too.
    invoke<string | null>("drag_title").then((t) => t && setTitle(t), () => {});
    const off = getCurrentWebviewWindow().listen<string>("host://drag-ghost", (e) => setTitle(e.payload));
    return () => void off.then((f) => f());
  }, []);
  return <div className="drag-ghost">{title}</div>;
}
