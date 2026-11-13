import { compile } from "./compile.ts";

export const seedSource =
  'import { rs } from "rustier"; console.log(rs`"Hello, world!".to_owned()`);';

export async function seed() {
  const result = await compile({
    typescript: seedSource,
  });

  if (result.type !== "ok") throw new Error(result.message);

  process.stdout.write(
    `Prepared playground (${result.duration.toFixed(0)}ms)\n`,
  );
}
