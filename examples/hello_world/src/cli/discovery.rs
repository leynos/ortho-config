use std::ffi::OsString;

use camino::Utf8PathBuf;

use crate::error::HelloWorldError;

pub(super) fn push_unique_candidate(candidates: &mut Vec<Utf8PathBuf>, path: Utf8PathBuf) {
    if path.as_str().is_empty() || candidates.contains(&path) {
        return;
    }
    candidates.push(path);
}

pub(super) fn collect_config_candidates() -> Vec<Utf8PathBuf> {
    let mut candidates = Vec::new();

    {
        let mut push = |path: Utf8PathBuf| push_unique_candidate(&mut candidates, path);
        add_explicit_config_path(&mut push);
        add_xdg_config_paths(&mut push);
        add_windows_config_paths(&mut push);
        add_home_config_paths(&mut push);
    }
    push_unique_candidate(&mut candidates, Utf8PathBuf::from(".hello_world.toml"));

    candidates
}

pub(super) fn add_explicit_config_path<F>(push_candidate: &mut F)
where
    F: FnMut(Utf8PathBuf),
{
    if let Ok(path) = std::env::var("HELLO_WORLD_CONFIG_PATH") {
        push_candidate(Utf8PathBuf::from(path));
    }
}

pub(super) fn add_xdg_config_paths<F>(push_candidate: &mut F)
where
    F: FnMut(Utf8PathBuf),
{
    let config_basename = ".hello_world.toml";

    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        let dir = Utf8PathBuf::from(dir);
        push_candidate(dir.join("hello_world").join("config.toml"));
        push_candidate(dir.join(config_basename));
    }

    if let Ok(dirs) = std::env::var("XDG_CONFIG_DIRS") {
        let os_dirs = OsString::from(&dirs);
        for dir in std::env::split_paths(&os_dirs) {
            if let Ok(dir) = Utf8PathBuf::from_path_buf(dir) {
                push_candidate(dir.join("hello_world").join("config.toml"));
                push_candidate(dir.join(config_basename));
            }
        }
    } else {
        #[cfg(unix)]
        {
            let dir = Utf8PathBuf::from("/etc/xdg");
            push_candidate(dir.join("hello_world").join("config.toml"));
            push_candidate(dir.join(config_basename));
        }
    }
}

pub(super) fn add_windows_config_paths<F>(push_candidate: &mut F)
where
    F: FnMut(Utf8PathBuf),
{
    let config_basename = ".hello_world.toml";

    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = Utf8PathBuf::from(appdata);
        push_candidate(dir.join("hello_world").join("config.toml"));
        push_candidate(dir.join(config_basename));
    }

    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let dir = Utf8PathBuf::from(local_appdata);
        push_candidate(dir.join("hello_world").join("config.toml"));
        push_candidate(dir.join(config_basename));
    }
}

pub(super) fn add_home_config_paths<F>(push_candidate: &mut F)
where
    F: FnMut(Utf8PathBuf),
{
    let config_basename = ".hello_world.toml";

    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"));
    if let Ok(home) = home {
        let home_path = Utf8PathBuf::from(home);
        push_candidate(
            home_path
                .join(".config")
                .join("hello_world")
                .join("config.toml"),
        );
        push_candidate(home_path.join(config_basename));
    }
}

pub(super) fn load_first_available_config(
    candidates: Vec<Utf8PathBuf>,
) -> Result<Option<ortho_config::figment::Figment>, HelloWorldError> {
    for path in candidates {
        match ortho_config::load_config_file(path.as_std_path()) {
            Ok(Some(figment)) => return Ok(Some(figment)),
            Ok(None) => {}
            Err(err) => return Err(HelloWorldError::Configuration(err)),
        }
    }
    Ok(None)
}

pub(super) fn discover_config_figment()
-> Result<Option<ortho_config::figment::Figment>, HelloWorldError> {
    let candidates = collect_config_candidates();
    load_first_available_config(candidates)
}
