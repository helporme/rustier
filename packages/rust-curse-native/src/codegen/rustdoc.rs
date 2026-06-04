pub mod lower;

use rustdoc_types::Crate as RCrate;

#[salsa::input]
pub struct Rustdoc {
    pub rcrate: RCrate,
}
