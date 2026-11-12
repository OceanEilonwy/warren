import { render } from "@solidjs/web";
import { App } from "./components/App";
import { start } from "./state";
import { installTooltips } from "./tooltip";
import "./style.css";

installTooltips();
render(() => <App />, document.getElementById("root")!);
void start();
