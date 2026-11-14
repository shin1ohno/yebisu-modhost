import { View, div } from "gpui-kit";
import { InputState, Input } from "gpui-base";
export default class Unicode extends View {
  init() { this.inputs = ["abc", "あ", "éab", "あ😀é"].map(value => InputState.new({value})); }
  render() { return div().size_full().children(this.inputs.map(input => Input.new(input))); }
}
