import { hi_copy, hi_ref, MsgRef } from "./greet.rs";

let ref = new MsgRef("world");

console.log(hi_ref(ref));
console.log(hi_copy({ name: "world" }));
