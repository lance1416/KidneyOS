use crate::sync::rwlock::sleep::RwLock;
use alloc::string::{String, ToString};
use spin::LazyLock;

pub static CURR_DIR: LazyLock<RwLock<String>> = LazyLock::new(|| RwLock::new("/".to_string()));
pub static HOST_NAME: LazyLock<RwLock<String>> =
    LazyLock::new(|| RwLock::new("kidney".to_string()));
