export interface InitOptions {
  dir?: string
}

export async function init({ dir }: InitOptions = {}): Promise<void> {
  console.log(`Hello world! Rustier init${dir ? ` in ${dir}` : ''}`)
}
