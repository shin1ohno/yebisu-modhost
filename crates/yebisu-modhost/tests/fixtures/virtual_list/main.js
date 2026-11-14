import { View, div } from "gpui-kit";
import { v_virtual_list } from "gpui-base";
// An empty VirtualList is the only list shape the prototype publishes; the
// parent rebuild gives the receiver a second view generation to key it under.
export default class EmptyList extends View {
  init(_props, cx) {
    this.n = 0; this.laidOut = false;
    cx.timer.every(100, (cx) => { if (this.laidOut && this.n === 0) { this.n = 1; cx.notify(); } });
  }
  render() {
    return div().size_full().on_size_change(() => { this.laidOut = true; })
      .child(v_virtual_list("probe-list", 0, [], (i) => String(i), (_range, _cx) => []))
      .child(`n=${this.n}`);
  }
}
