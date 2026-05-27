//! Workspace-relative path helpers for the self-contained SlugArch monorepo.

use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    workspace_root_from(env!("CARGO_MANIFEST_DIR"))
}

pub fn workspace_root_from(start: impl AsRef<Path>) -> PathBuf {
    let mut dir = start.as_ref();
    loop {
        if is_workspace_root(dir) {
            return dir.to_path_buf();
        }
        dir = dir.parent().unwrap_or_else(|| {
            panic!(
                "could not find SlugArch workspace root above {}",
                start.as_ref().display()
            )
        });
    }
}

pub fn vendor_dir() -> PathBuf {
    workspace_root().join("vendor")
}

pub fn gemma_generated_root() -> PathBuf {
    vendor_dir().join("gemma-generated")
}

pub fn concordia_ptx_root() -> PathBuf {
    vendor_dir().join("concordia-ptx")
}

pub fn fixture(name: impl AsRef<Path>) -> PathBuf {
    workspace_root().join("tests").join("fixtures").join(name)
}

fn is_workspace_root(path: &Path) -> bool {
    path.join("Cargo.toml").is_file()
        && path.join("vendor").join("gemma-generated").is_dir()
        && path.join("vendor").join("concordia-ptx").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_workspace_root_from_crate() {
        let root = workspace_root();
        assert!(root.join("Cargo.toml").is_file());
        assert!(root.join("vendor/gemma-generated").is_dir());
        assert!(root.join("vendor/concordia-ptx").is_dir());
    }

    #[test]
    fn builds_fixture_paths() {
        assert!(fixture("gemm.ptx").is_file());
    }
}
