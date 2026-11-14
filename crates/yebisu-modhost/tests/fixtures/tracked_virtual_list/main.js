import { View, div } from "gpui-kit";
import { v_virtual_list, VirtualListScrollHandle } from "gpui-base";
// The same empty list as virtual_list, driven by a scroll handle the script
// owns: the receiver must see the list bound to that handle's entity.
export default class TrackedList extends View {
  init(_props, cx) {
    this.n = 0; this.laidOut = false;
    this.scroll = VirtualListScrollHandle.new();
    cx.timer.every(100, (cx) => { if (this.laidOut && this.n === 0) { this.n = 1; cx.notify(); } });
  }
  render() {
    return div().size_full().on_size_change(() => { this.laidOut = true; })
      .child(v_virtual_list("tracked-list", 0, [], (i) => String(i), (_range, _cx) => []).track_scroll(this.scroll))
      .child(`n=${this.n}`);
  }
}
