use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct User {
    pub name: String,
}

/// Greets user by name
pub fn greet(user: User) -> String {
    format!("Hello from rust, {0}!", user.name)
}
