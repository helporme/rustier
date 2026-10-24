import { rs } from "rustier";

export function greet(): string {
  return rs`
    let js = ${undefined};
    js.env().new_string(library_example::greet().unwrap())
  `;
}

console.log(greet());
