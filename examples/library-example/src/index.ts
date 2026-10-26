import { rs } from "rustier";

export function greet(): string {
  return rs`
    println!("hi");
    ${undefined}
  `;
}

greet();
