import { View, div } from "gpui-kit";
import { InputState, Input, v_virtual_list } from "gpui-base";
// An untracked VirtualList beside an Input changed by a host setter. The setter
// does not rebuild the parent, so the runner compares retained state against
// the previous publication, which carries the list's wire-only scroll entity.
// The first set writes the value the Input already has: its frame changes
// nothing, so the comparison runs past the Input to the list's entity.
export default class ListHostSet extends View {
  init(_props, cx) {
    this.input = InputState.new({value: "before"}); this.renders = 0; this.laidOut = false;
    let step = 0;
    cx.timer.every(100, () => {
      if (!this.laidOut) return;
      if (step === 0) this.input.set_value("before");
      if (step === 1) this.input.set_value("after");
      step++;
    });
  }
  render() {
    this.renders++;
    return div().size_full().on_size_change(() => { this.laidOut = true; })
      .child(v_virtual_list("host-list", 0, [], (i) => String(i), (_range, _cx) => []))
      .child(Input.new(this.input)).child(`render=${this.renders}`);
  }
}
