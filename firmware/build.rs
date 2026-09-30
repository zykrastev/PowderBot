use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read web assets") {
        let path = entry.expect("asset entry").path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}
fn main() {
    embuild::espidf::sysenv::output();
    println!("cargo:rerun-if-changed=web");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("web");
    let mut files = Vec::new();
    collect(&root, &mut files);
    files.sort();
    let mut output = String::from("const ASSETS: &[(&str, &str, &[u8])] = &[\n");
    for file in files {
        let name = file
            .strip_prefix(&root)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        let route = if name == "console.html" {
            "/console".to_owned()
        } else {
            format!("/{name}")
        };
        let mime = match file.extension().and_then(|e| e.to_str()).unwrap_or("") {
            "html" => "text/html; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "js" => "text/javascript; charset=utf-8",
            "svg" => "image/svg+xml",
            "woff2" => "font/woff2",
            _ => "application/octet-stream",
        };
        output.push_str(&format!(
            "({route:?}, {mime:?}, include_bytes!({:?})),\n",
            file.to_str().unwrap()
        ));
    }
    output.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("web_assets.rs"),
        output,
    )
    .unwrap();
}
