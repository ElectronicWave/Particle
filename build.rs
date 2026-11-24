use std::{env, fs, io};

const CLIENT_ID_ENV: &str = "PARTICLE_MICROSOFT_CLIENT_ID";
const ENV_FILE: &str = ".env";

fn main() {
    println!("cargo:rerun-if-changed={ENV_FILE}");
    println!("cargo:rerun-if-env-changed={CLIENT_ID_ENV}");

    let value = env::var(CLIENT_ID_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .or_else(read_env_file);
    println!(
        "cargo:rustc-env={CLIENT_ID_ENV}={}",
        value.unwrap_or_default()
    );
}

fn read_env_file() -> Option<String> {
    let content = match fs::read_to_string(ENV_FILE) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => panic!("failed to read {ENV_FILE}: {error}"),
    };

    parse_env(&content, CLIENT_ID_ENV)
}

fn parse_env(content: &str, key: &str) -> Option<String> {
    for (index, raw) in content.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((name, value)) = line.split_once('=') else {
            panic!("invalid {ENV_FILE} line {}: expected KEY=VALUE", index + 1);
        };
        if name.trim() != key {
            continue;
        }

        let value = unquote(value.trim());
        if !value.is_empty() {
            return Some(value);
        }
    }

    None
}

fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
        .to_owned()
}
