use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
pub fn data_dir() -> anyhow::Result<PathBuf> {
    #[cfg(windows)]
    {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("com.user.winmux"))
            .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA unavailable"));
    }
    #[cfg(target_os = "macos")]
    {
        return home_dir()
            .map(|path| path.join("Library/Application Support/com.user.winmux"))
            .ok_or_else(|| anyhow::anyhow!("HOME unavailable"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Ok(std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|path| path.join(".local/share")))
            .ok_or_else(|| anyhow::anyhow!("Data directory unavailable"))?
            .join("com.user.winmux"))
    }
}
pub fn prepare_environment() {
    #[cfg(target_os = "macos")]
    {
        // Finder does not inherit a login shell's PATH. Include conventional CLI
        // install locations without running user shell scripts during GUI startup.
        let mut paths: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|value| std::env::split_paths(&value).collect())
            .unwrap_or_default();
        let mut candidates = vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ];
        if let Some(home) = home_dir() {
            candidates.push(home.join(".local/bin"));
            candidates.push(home.join(".cargo/bin"));
        }
        for path in candidates.into_iter().rev() {
            if !paths.contains(&path) {
                paths.insert(0, path);
            }
        }
        if let Ok(value) = std::env::join_paths(paths) {
            std::env::set_var("PATH", value);
        }
    }
}
