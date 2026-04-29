// Renderers — typed Lisp form → multi-repo artifact bundle.
//
// Each render emits one or more files into an output directory; the
// caller (operator or `feira cluster apply`) places each file into its
// destination repo. Filenames are prefixed by their target repo so it's
// unambiguous (e.g., `nix-fleet-domains.nix.fragment`,
// `vigia-mar-helmrelease.yaml`).

pub mod cluster;
pub mod fleet;
pub mod policy;

use std::fs;
use std::io;
use std::path::Path;

/// Write one rendered artifact to disk under `out_dir/name`.
pub fn write_artifact(out_dir: &Path, name: &str, content: &str) -> io::Result<()> {
    fs::create_dir_all(out_dir)?;
    let path = out_dir.join(name);
    fs::write(&path, content)?;
    Ok(())
}
