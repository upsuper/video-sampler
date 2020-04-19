use anyhow::{ensure, Context, Result};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<()> {
    let project_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let res_dir = project_dir.join("res");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let status = Command::new("glib-compile-resources")
        .arg(param_with_path("--sourcedir=", &res_dir))
        .arg(param_with_path("--target=", &out_dir.join("res.gresource")))
        .arg(res_dir.join("spec.gresource.xml"))
        .status()
        .context("failed to run glib-compile-resources")?;
    ensure!(status.success(), "glib-compile-resources must succeed");
    for entry in res_dir.read_dir()? {
        let entry = entry?;
        println!("cargo:rerun-if-changed={}", entry.path().display());
    }
    Ok(())
}

fn param_with_path(prefix: &str, path: &Path) -> OsString {
    let mut param = OsString::from(prefix);
    param.push(path.as_os_str());
    param
}
