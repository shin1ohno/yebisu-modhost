import { View, div } from "gpui-kit";
import { InputState, Input } from "gpui-base";
export default class Subscriptions extends View {
  init(_props, cx) {
    this.input = InputState.new({value: "unchanged"}); this.renders = 0;
    cx.timer.after(250, () => { this.input.on("change", () => {}); });
    cx.timer.after(550, () => { this.input.on("change", () => {}); });
    // The setter notifies even when its exported config stays unchanged.
    cx.timer.after(800, () => { this.input.set_masked(false); });
    cx.timer.after(1100, () => { this.input.on("submit", () => {}); });
    cx.timer.after(1400, () => { this.input.set_masked(true); });
  }
  render() { this.renders++; return div().size_full().child(Input.new(this.input)).child(`render=${this.renders}`); }
}
