import { View, div } from "gpui-kit";
import { InputState, Input } from "gpui-base";
// Steps start only after the first frame was laid out, so the first
// publication always carries the untouched Input. After that no step
// notifies the parent, and only Input-local setters and subscriptions
// change state: the assertions hold whichever steps share a frame.
export default class Subscriptions extends View {
  init(_props, cx) {
    this.input = InputState.new({value: "unchanged"}); this.renders = 0; this.laidOut = false;
    const steps = [
      // The setter notifies even though the exported config stays the same.
      () => this.input.set_masked(false),
      () => this.input.on("change", () => {}),
      () => this.input.on("change", () => {}),
      () => this.input.on("submit", () => {}),
    ];
    cx.timer.every(100, () => { if (this.laidOut && steps.length > 0) steps.shift()(); });
  }
  render() {
    this.renders++;
    return div().size_full().on_size_change(() => { this.laidOut = true; })
      .child(Input.new(this.input)).child(`render=${this.renders}`);
  }
}
