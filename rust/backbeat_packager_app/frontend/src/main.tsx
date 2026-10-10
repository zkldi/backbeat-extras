import { render } from "solid-js/web";

import "./base.css";
import { App } from "./App";
import "./styles.css";
import { initTheme } from "./theme";

initTheme();

render(() => <App />, document.getElementById("root")!);
