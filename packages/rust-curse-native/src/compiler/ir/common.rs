use crate::compiler::FilePath;

pub(super) fn compile_relative_path(
    from_mod_path: &FilePath,
    to_mod_path: &FilePath,
) -> Option<String> {
    let from_mod_path = from_mod_path.as_path();
    let to_mod_path = to_mod_path.as_path();
    let mut from_components = from_mod_path.parent()?.components().peekable();
    let mut to_components = to_mod_path.components().peekable();

    while matches!(
        (from_components.peek(), to_components.peek()),
        (Some(from), Some(to)) if from == to
    ) {
        from_components.next();
        to_components.next();
    }

    if from_components.clone().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        )
    }) || to_components.clone().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        )
    }) {
        return None;
    }

    let mut path = String::new();
    let parent_count = from_components.count();
    if parent_count == 0 {
        path.push_str("./");
    } else {
        for _ in 0..parent_count {
            path.push_str("../");
        }
    }

    for (index, component) in to_components.enumerate() {
        if index > 0 {
            path.push('/');
        }
        path.push_str(&component.as_os_str().to_string_lossy());
    }

    Some(path)
}
