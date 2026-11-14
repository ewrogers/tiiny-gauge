use std::{env, fs, io, path::PathBuf};

fn main() -> io::Result<()> {
    let out_dir = env::var_os("OUT_DIR").ok_or(io::Error::other("Cargo did not set OUT_DIR"))?;
    let out_dir = PathBuf::from(out_dir);
    fs::write(out_dir.join("memory.x"), include_bytes!("memory.x"))?;
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rerun-if-changed=memory.x");
    Ok(())
}
