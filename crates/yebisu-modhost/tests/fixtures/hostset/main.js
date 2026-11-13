import { View, div } from "gpui-kit";
import { InputState, Input } from "gpui-base";
export default class HostSet extends View {
  init(_props, cx) {
    this.input = InputState.new({value: "before"}); this.renders = 0;
    cx.timer.after(300, () => { this.input.set_value("after😀"); });
    cx.timer.after(650, () => { this.input.set_masked(true); });
    cx.timer.after(1000, () => { this.input.set_selected_range({start: 0, end: 5}); });
  }
  render() { this.renders++; return div().size_full().child(Input.new(this.input)).child(`render=${this.renders}`); }
}
