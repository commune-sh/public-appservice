use std::{path::PathBuf, sync::LazyLock};

pub static DEFAULT_CONFIG_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| [env!("CARGO_PKG_NAME"), "config.toml"].iter().collect());
