import { View, div } from "gpui-kit";
export default class Counter extends View {
  init(_props, cx) {
    this.count = 0; this.sized = false;
    cx.timer.every(100, (cx) => {
      if (this.count < 3) { this.count++; cx.notify(); }
    });
  }
  render() { return div().size_full().on_size_change((_size, cx) => { this.sized = true; cx.notify(); }).child(`timer-count=${this.count} sized=${this.sized}`); }
}
