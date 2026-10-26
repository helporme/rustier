import { rs } from "rustier";

export function greet(): string {
  return rs`
    let js = ${undefined};
    js.env().new_string("Hello world!")
  `;
}

console.log(greet());
