use std::{env, fs, io, path::PathBuf};

fn main() -> io::Result<()> {
    let out_dir = env::var_os("OUT_DIR").ok_or(io::Error::other("Cargo did not set OUT_DIR"))?;
    let out_dir = PathBuf::from(out_dir);
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("arm") {
        fs::write(out_dir.join("memory.x"), include_bytes!("pico-memory.x"))?;
        println!("cargo:rustc-link-search={}", out_dir.display());
    }
    println!("cargo:rerun-if-changed=pico-memory.x");
    if env::var_os("CARGO_FEATURE_BAKED_CONFIG").is_none() {
        return Ok(());
    }
    println!("cargo:rerun-if-changed=.env");
    let local = match fs::read_to_string(".env") {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let mut config = String::new();
    for (name, fallback) in [
        ("WIFI_SSID", ""),
        ("WIFI_PASSWORD", ""),
        ("TIINY_IP", ""),
        ("TIINY_API_KEY", ""),
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        let value = env::var(name).ok().unwrap_or_else(|| {
            local
                .lines()
                .filter_map(|line| line.split_once('='))
                .find(|(key, _)| key.trim() == name)
                .map_or(fallback, |(_, value)| value.trim())
                .trim_matches('"')
                .to_owned()
        });
        config.push_str(&format!("pub const {name}: &str = {value:?};\n"));
    }
    fs::write(out_dir.join("local_config.rs"), config)?;
    Ok(())
}
