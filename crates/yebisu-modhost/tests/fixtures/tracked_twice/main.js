import { View, div } from "gpui-kit";
import { v_virtual_list, VirtualListScrollHandle } from "gpui-base";
// The list calls track_scroll twice. The shell follows the last handle, so the
// receiver must see the list bound to `second`, not `first`.
export default class TrackedTwice extends View {
  init(_props, _cx) {
    this.first = VirtualListScrollHandle.new();
    this.second = VirtualListScrollHandle.new();
  }
  render() {
    return div().size_full()
      .child(v_virtual_list("twice-list", 0, [], (i) => String(i), (_range, _cx) => [])
        .track_scroll(this.first).track_scroll(this.second))
      .child(`first=${this.first.__handle}`)
      .child(`second=${this.second.__handle}`);
  }
}
