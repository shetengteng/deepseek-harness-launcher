fn main() {
    println!("cargo:rerun-if-env-changed=DSH1024_CATALOG_ENDPOINT");
    tauri_build::build()
}
