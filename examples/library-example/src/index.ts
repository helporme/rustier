import { rs } from 'rustier';

const value = rs`
  fn get() -> Result<u32, String> { Ok(10) }
  let x = get()?;
  Ok(x * 2)
`;

console.log(value) // 20

const value2 = rs`
  fn get() -> Result<u32, String> { Err("Hello from rust!".to_owned()) }
  let x = get()?;
  Ok(x * 2)
`; // throws RustError("Hello from rust")


console.log(value2) // 20
