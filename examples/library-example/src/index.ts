import { greet } from "./lib.rs";

let value = greet();
if (value.type === "some") {
  console.log(value[0])
}
