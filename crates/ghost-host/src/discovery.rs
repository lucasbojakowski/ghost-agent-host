use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub fn default_clap_directories() -> Vec<PathBuf> {
    let mut paths = BTreeSet::new();
    if cfg!(target_os = "windows") {
        for variable in ["COMMONPROGRAMFILES", "LOCALAPPDATA", "APPDATA"] {
            if let Some(root) = std::env::var_os(variable) {
                let root = PathBuf::from(root);
                let path = if variable == "LOCALAPPDATA" {
                    root.join("Programs").join("Common").join("CLAP")
                } else {
                    root.join("CLAP")
                };
                paths.insert(path);
            }
        }
    } else {
        paths.insert(PathBuf::from("/usr/lib/clap"));
        paths.insert(PathBuf::from("/usr/local/lib/clap"));
    }
    if let Some(extra) = std::env::var_os("CLAP_PATH") {
        paths.extend(std::env::split_paths(&extra));
    }
    paths.into_iter().collect()
}

pub fn discover_clap_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = BTreeSet::new();
    for root in roots {
        visit(root, 0, &mut found);
    }
    found.into_iter().collect()
}

fn visit(path: &Path, depth: usize, found: &mut BTreeSet<PathBuf>) {
    if depth > 8 {
        return;
    }
    let Ok(metadata) = path.metadata() else {
        return;
    };
    if metadata.is_file() {
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("clap"))
        {
            found.insert(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        visit(&entry.path(), depth + 1, found);
    }
}
