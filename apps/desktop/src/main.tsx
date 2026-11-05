import "@terminescu/ui/styles.css";
import "./app.css";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { connect } from "./connect";
import { Ghost } from "./Ghost";

const root = createRoot(document.getElementById("root")!);
// The drag ghost (task 8.1) is no workspace window: it never connects to the core's state.
if (location.hash === "#ghost") root.render(<Ghost />);
else
  connect()
    .then((c) => root.render(<App {...c} />))
    .catch((e: unknown) => root.render(<pre className="boot-error">{String(e)}</pre>));
