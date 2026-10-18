export const examples = [
	{
		id: 'hello',
		name: 'Hello, friend',
		rust: `pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}`,
		typescript: `import { greet } from './lib.rs';

const message = greet('friend');
console.log(message);`
	},
	{
		id: 'counter',
		name: 'A little counter',
		rust: `pub struct Counter {
    pub value: i32,
}

impl Counter {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn increment(&mut self) {
        self.value += 1;
    }
}`,
		typescript: `import { Counter } from './lib.rs';

const counter = new Counter();

counter.increment();
counter.increment();

console.log(counter.value);
// A Rust struct. Just like that.`
	},
	{
		id: 'enums',
		name: 'Working with enums',
		rust: `pub enum Mood {
    Curious,
    Happy,
}

pub fn current_mood() -> Mood {
    Mood::Happy
}`,
		typescript: `import { current_mood } from './lib.rs';

const mood = current_mood();
console.log(mood.type);`
	},
	{
		id: 'numbers',
		name: 'Working with numbers',
		rust: `pub fn double(value: i32) -> i32 {
    value * 2
}`,
		typescript: `import { double } from './lib.rs';

const values = [1, 2, 3, 4];
console.log(values.map(double));`
	}
] as const;
