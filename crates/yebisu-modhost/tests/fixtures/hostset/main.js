import { View, div } from "gpui-kit";
import { InputState, Input } from "gpui-base";
// The test substitutes one host setter for the marker below. It runs only
// after the first frame was laid out, so the first publication always shows
// the untouched Input and the setter's own frame request is the only wake.
export default class HostSet extends View {
  init(_props, cx) {
    this.input = InputState.new({value: "before"}); this.renders = 0; this.laidOut = false;
    let pending = true;
    cx.timer.every(100, () => {
      if (this.laidOut && pending) { pending = false; /*SETTER*/; }
    });
  }
  render() {
    this.renders++;
    return div().size_full().on_size_change(() => { this.laidOut = true; })
      .child(Input.new(this.input)).child(`render=${this.renders}`);
  }
}
