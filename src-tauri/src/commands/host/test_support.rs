use std::path::Path;

pub fn write_node_runtime(runtime_dir: &Path, version: &str) {
    let node_dir = runtime_dir.join(format!("node-v{version}"));
    let node_path = crate::node::install::node_bin_path(&node_dir);
    std::fs::create_dir_all(node_path.parent().unwrap()).unwrap();
    std::fs::write(node_path, "node").unwrap();
    std::fs::write(runtime_dir.join("VERSION"), version).unwrap();
}

pub fn write_dsh_entry(dsh_dir: &Path, version: &str) {
    let entry = dsh_dir
        .join(version)
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join(crate::dsh::DSH_ENTRY_REL);
    std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
    std::fs::write(entry, "// dsh").unwrap();
}
