use std::fs;
use std::path::Path;

pub fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_abs_path = entry.path();
        let dst_abs_path = dst.join(entry.file_name());
        if src_abs_path.is_dir() {
            if entry.file_name() == "target" {
                continue;
            }
            copy_dir_recursive(&src_abs_path, &dst_abs_path);
        } else {
            fs::copy(&src_abs_path, &dst_abs_path).unwrap();
        }
    }
}
