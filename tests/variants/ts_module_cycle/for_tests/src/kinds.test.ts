import { Stream } from "./kinds";
import { listen } from "./socket";

const stream: Stream | undefined = listen(1);
if (!stream?.accept()) {
  throw new Error("accepts up to the backlog");
}
