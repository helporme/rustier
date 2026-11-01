import { rs } from "rustier";

export function greet(): string {
  return rs`"hello".to_owned()`;
}

console.log(greet());
