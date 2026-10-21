export const examples = [
	{
		id: 'hello',
		name: 'Hello world',
		topic: 'Getting started',
		rust: `pub fn greet(name: &str) -> String {
    format!("Hello {name}!")
}`,
		typescript: `import { greet } from './lib.rs'

console.log(greet('World'))`
	},
	{
		id: 'structs-and-enums',
		name: 'Structs and enums',
		topic: 'Structs and enums',
		rust: `pub struct Sheep {
    pub wool: u32,
}

pub enum Animal {
    Slime { size: u32 },
    Sheep(Sheep),
    Bug,
}

pub fn encounter() -> Animal {
    Animal::Sheep(Sheep { wool: 10 })
}`,
		typescript: `import { encounter } from './lib.rs'

const animal = encounter()

switch (animal.type) {
  case 'Slime':
    animal.size += 1
    console.log(animal.size)
    break
  case 'Sheep':
    console.log(animal[0].wool)
    break
  case 'Bug':
    console.log('A bug!')
    break
}`
	},
	{
		id: 'borrowing-and-moving',
		name: 'Borrowing and moving',
		topic: 'Structs and enums',
		rust: `#[derive(Clone)]
pub struct Sheep {
    pub wool: u32,
}

pub fn adopt() -> Sheep {
    Sheep { wool: 10 }
}

pub fn is_fluffy(sheep: &Sheep) -> bool {
    sheep.wool >= 5
}

// Sheep implements Clone, so it can be
// passed by value from JS.
// Without Clone, this function is skipped
// with a diagnostic.
pub fn shear(mut sheep: Sheep) -> Sheep {
    sheep.wool = 0;

    sheep
}`,
		typescript: `import {
  adopt,
  is_fluffy,
  shear,
} from './lib.rs'

// Keep this handle to work with the Sheep
// stored in Rust.
const sheep = adopt()

// A reference parameter avoids cloning
// the Sheep.
console.log(is_fluffy(sheep)) // true

// You can keep using sheep: this by-value
// call receives a clone.
const shorn = shear(sheep)
console.log(shorn.wool) // 0
console.log(sheep.wool) // 10 — still usable`
	},
	{
		id: 'field-access',
		name: 'Field access',
		topic: 'Structs and enums',
		rust: `#[derive(Clone)]
pub struct Sheep {
    pub wool: u32,
}

impl Sheep {
    pub fn shear(&mut self) -> u32 {
        std::mem::take(&mut self.wool)
    }
}

pub struct Pasture {
    pub sheep: Sheep,
}

impl Pasture {
    pub fn new() -> Self {
        Self {
            sheep: Sheep { wool: 10 },
        }
    }
}`,
		typescript: `import { Pasture } from './lib.rs'

const pasture = new Pasture()

// This refers to the Sheep inside pasture,
// so changes affect pasture.
const old_sheep = pasture.sheep
old_sheep.wool = 5
console.log(pasture.sheep.wool) // 5

// shorthand to \`Sheep { wool: 10 }\`
pasture.sheep = { wool: 10 }
console.log(pasture.sheep.wool) // 10

// This refers to the new field; old_sheep
// still holds the previous value.
const sheep = pasture.sheep
console.log(old_sheep.wool) // 5
console.log(sheep.wool)     // 10

// This mutates the Sheep in pasture.
console.log(sheep.shear())      // 10
console.log(pasture.sheep.wool) // 0
console.log(old_sheep.wool)     // 5`
	},
	{
		id: 'generic-types',
		name: 'Generic types',
		topic: 'Generics',
		rust: `pub fn item_count(
    items: Vec<String>,
) -> u32 {
    items.len() as u32
}

pub fn total_items(
    bags: Vec<Vec<String>>,
) -> u32 {
    bags.iter()
        .map(|bag| bag.len() as u32)
        .sum()
}`,
		typescript: `import {
  item_count,
  total_items,
} from './lib.rs'
import { Vec } from '$rust/alloc/vec'

const Strings = Vec('String')
const items = new Strings()
items.push('healing potion')
items.push('dried mushrooms')
console.log(item_count(items)) // 2

const Bags = Vec(Strings)
const bags = new Bags()
bags.push(items)
console.log(total_items(bags)) // 2`
	},
	{
		id: 'generic-impls',
		name: 'Generic impls with trait bounds',
		topic: 'Generics',
		rust: `pub struct Pouch<T> {
    pub item: T,
}

impl<T> Pouch<T> {
    // For Pouch<String>, this becomes
    // new(item: String).
    pub fn new(item: T) -> Self {
        Self { item }
    }
}

impl<T> Pouch<T>
where
    T: Clone,
{
    // Available for Pouch<String> because
    // String implements Clone.
    pub fn duplicate(&self) -> Self {
        Self::new(self.item.clone())
    }
}

impl<T> Pouch<T>
where
    T: Default,
{
    // For Pouch<String>, returns a String
    // and leaves an empty string.
    pub fn take(&mut self) -> T {
        std::mem::take(&mut self.item)
    }
}

pub fn starter_pack() -> Pouch<String> {
    Pouch::new("healing potion".to_owned())
}`,
		typescript: `import {
  Pouch,
  starter_pack,
} from './lib.rs'

const pouch = starter_pack()
const backup = pouch.duplicate()

console.log(pouch.take())   // healing potion
console.log(pouch.item)     // ""
console.log(backup.item)    // healing potion

const StringPouch = Pouch('String')
const snacks = new StringPouch(
  'dried mushrooms',
)
console.log(snacks.take()) // dried mushrooms`
	},
	{
		id: 'object-literals',
		name: 'Construction from object literals',
		topic: 'Structs and enums',
		rust: `#[derive(Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn distance_to(
        &self,
        other: &Point,
    ) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;

        dx.hypot(dy)
    }
}`,
		typescript: `import { Point } from './lib.rs'

const camp = Point.of({ x: 0, y: 0 })
const pond = Point.of({ x: 3, y: 4 })

console.log(camp.distance_to(pond)) // 5
console.log(
  camp.distance_to({ x: 3, y: 4 }),
) // 5
console.log(Point.of(camp) === camp) // true`
	},
	{
		id: 'diagnostics',
		name: 'Diagnostics',
		topic: 'Diagnostics',
		rust: `pub async fn summon() -> String {
    "A slime appears!".to_owned()
}`,
		typescript: `// Async functions are not supported.
// Enable Inspect to see why summon is skipped.`
	}
] as const;

export const exampleGroups = Array.from(
	new Set(examples.map((example) => example.topic)),
	(topic) => ({
		topic,
		examples: examples.filter((example) => example.topic === topic)
	})
);
