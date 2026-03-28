mod messages;

pub use messages::*;

pub fn bye(name: String) -> String {
    format!("Bye, {0}!", name)
}
