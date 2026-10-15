use std::{
    env, fs,
    path::{Path, PathBuf},
};
fn collect(root: &Path, dir: &Path, entries: &mut Vec<(String, PathBuf)>) {
    if !dir.exists() {
        return;
    }
    for entry in fs::read_dir(dir).expect("read mobile asset directory") {
        let entry = entry.expect("read mobile asset");
        let kind = entry.file_type().expect("mobile asset type");
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            collect(root, &path, entries);
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            entries.push((relative, path));
        }
    }
}
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../dist-mobile");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut entries = Vec::new();
    collect(&root, &root, &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut generated = String::from("static MOBILE_ASSETS: &[(&str, &[u8])] = &[\n");
    if !root.join("mobile.html").is_file() {
        println!("cargo:warning=Mobile assets unavailable; run pnpm build before building a usable mobile app");
    }
    for (relative, path) in entries {
        generated.push_str(&format!("({relative:?}, include_bytes!({:?})),\n", path));
    }
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("mobile_assets.rs"),
        generated,
    )
    .expect("write mobile asset index");
    tauri_build::build()
}
