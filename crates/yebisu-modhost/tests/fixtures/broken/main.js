import { View } from "gpui-kit";
// Fails in render, after the application has started.
export default class Broken extends View {
  render() { throw new Error("broken-render"); }
}
