import "@terminescu/ui/styles.css";
import "./app.css";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { connect } from "./connect";

const root = createRoot(document.getElementById("root")!);
connect()
  .then(({ windowId, host, workspace }) => root.render(<App windowId={windowId} host={host} workspace={workspace} />))
  .catch((e: unknown) => root.render(<pre className="boot-error">{String(e)}</pre>));
