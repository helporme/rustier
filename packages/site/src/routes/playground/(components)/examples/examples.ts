export const examples = [
	{
		id: 'word-count',
		name: 'Word count',
		topic: 'Getting started',
		typescript: `import { rs } from 'rustier'

const text = 'The quick brown fox'

const words = rs\`
    let text: String = \${text};
    text.split_whitespace().count() as u32
\`

console.log(words)`
	},
	{
		id: 'hello-world',
		name: 'Hello world',
		topic: 'Getting started',
		typescript: `import { rs } from 'rustier'

const name = 'World'
const greeting = rs\`
    let name: String = \${name};
    format!("Hello {name}!")
\`

console.log(greeting)`
	},
	{
		id: 'numbers',
		name: 'Numbers',
		topic: 'Getting started',
		typescript: `import { rs } from 'rustier'

const square = (value: number) => rs\`
    let value: f64 = \${value};
    value * value
\`

console.log(square(12))
console.log(rs\`(3.0f64).hypot(4.0)\`)`
	},
	{
		id: 'borrowing-and-moving',
		name: 'Borrowing and moving',
		topic: 'Ownership',
		typescript: `import { rs } from 'rustier'

const values = rs\`vec![1u32, 2, 3]\`

rs\`
    let values: &mut Vec<u32> = \${values};
    values.push(4);
\`

console.log(rs\`
    let values: &Vec<u32> = \${values};
    values.len() as u32
\`)

console.log(rs\`
    let values: Vec<u32> = \${values};
    values.into_iter().sum::<u32>()
\`)

// Passing by value moves the vector. The old handle is no longer usable.
try {
    rs\`let values: &Vec<u32> = \${values}; values.len() as u32\`
} catch (error) {
    console.log(String(error))
}`
	},
	{
		id: 'generic-types',
		name: 'Collections',
		topic: 'Ownership',
		typescript: `import { rs } from 'rustier'

const items = rs\`Vec::<String>::new()\`

for (const item of ['healing potion', 'dried mushrooms']) {
    rs\`
        let items: &mut Vec<String> = \${items};
        let item: String = \${item};
        items.push(item);
    \`
}

console.log(rs\`
    let items: &Vec<String> = \${items};
    items.join(", ")
\`)`
	},
	{
		id: 'results',
		name: 'Results',
		topic: 'Error handling',
		typescript: `import { rs } from 'rustier'

const parse = (text: string) => rs\`
    let text: String = \${text};
    text.parse::<u32>()
\`

console.log(parse('42'))

try {
    parse('not a number')
} catch (error) {
    console.log(String(error))
}`
	},
	{
		id: 'diagnostics',
		name: 'Diagnostics',
		topic: 'Error handling',
		typescript: `import { rs } from 'rustier'

// Run to see the compiler diagnostic, then fix the Rust expression.
console.log(rs\`2 + ""\`)`
	}
] as const;

export const exampleGroups = Array.from(
	new Set(examples.map((example) => example.topic)),
	(topic) => ({
		topic,
		examples: examples.filter((example) => example.topic === topic)
	})
);
