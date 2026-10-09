//@ api: node
//@ cargo-check

#[derive(Clone)]
pub struct Type0;

pub fn direct_then_indirect(argument_0: String, argument_1: self::Type0) {
    let _ = argument_0;
    let _ = argument_1;
}

pub fn direct_then_indirect_with_return(
    argument_0: String,
    argument_1: self::Type0,
) -> String {
    let _ = argument_1;
    argument_0
}

pub fn nested_indirect(argument_0: self::Type0, argument_1: self::Type0) {
    let _ = argument_0;
    let _ = argument_1;
}
