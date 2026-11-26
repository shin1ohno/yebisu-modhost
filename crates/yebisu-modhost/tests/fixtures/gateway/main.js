import { View, div } from "gpui-kit";
import { run, setAgentOrder, openOverlay, closeOverlay, notify } from "herdr";
// Each gateway call's failure, as the mod sees it.
function attempt(name, call) {
  try { call(); return `${name}=ok`; } catch (e) { return `${name}=${String(e)}`; }
}
export default class Gateway extends View {
  init() {
    this.results = [
      attempt("run", () => run("pane.split")),
      attempt("order", () => setAgentOrder(["a", "b"])),
      attempt("open", () => openOverlay({ mod: "other" })),
      attempt("close", () => closeOverlay("overlay-1")),
      attempt("notify", () => notify({ message: "done" })),
    ];
  }
  render() { return div().child(this.results.join(" ")); }
}
