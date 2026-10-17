use crate::fsutil::write_atomic;
use crate::paths::AppPaths;
use crate::platform::{link_dir, link_or_copy_file};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Folders every instance shares through a directory link (spec 6.2).
pub const SHARED_DIRS: [&str; 4] = ["resourcepacks", "shaderpacks", "screenshots", "config"];

#[derive(Debug, Clone)]
pub struct PreparedInstance {
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
    servers_snapshot: Option<SystemTime>,
}

/// Builds `instances/<id>/` for one launch: shared folders linked, mods mirrored,
/// server list copied in, options.txt seeded if missing.
pub fn prepare_instance(
    paths: &AppPaths,
    account_id: &str,
    mod_jars: &[PathBuf],
    frog_jar: Option<&Path>,
    options_template: Option<&Path>,
) -> io::Result<PreparedInstance> {
    let game_dir = paths.instance(account_id);
    let natives_dir = game_dir.join("natives");
    fs::create_dir_all(&natives_dir)?;

    for dir in SHARED_DIRS {
        link_dir(&paths.shared_gamedata().join(dir), &game_dir.join(dir))?;
    }

    let mods = game_dir.join("mods");
    if mods.is_dir() {
        for entry in fs::read_dir(&mods)? {
            let path = entry?.path();
            if path.is_file() {
                fs::remove_file(path)?;
            }
        }
    }
    fs::create_dir_all(&mods)?;
    for jar in mod_jars.iter().map(PathBuf::as_path).chain(frog_jar) {
        if let Some(name) = jar.file_name() {
            link_or_copy_file(jar, &mods.join(name))?;
        }
    }

    // Minecraft rewrites servers.dat by replacing the file, which breaks hard links,
    // so each instance gets a copy and publishes it back on exit if it changed.
    let shared_servers = paths.shared_gamedata().join("servers.dat");
    let servers = game_dir.join("servers.dat");
    if shared_servers.is_file() {
        fs::copy(&shared_servers, &servers)?;
    }
    let servers_snapshot = modified(&servers);

    let options = game_dir.join("options.txt");
    if let Some(template) = options_template {
        if !options.exists() && template.is_file() {
            fs::copy(template, &options)?;
        }
    }

    Ok(PreparedInstance { game_dir, natives_dir, servers_snapshot })
}

/// After the game exits: publish this instance's server list if it changed during the session.
pub fn finish_instance(paths: &AppPaths, prepared: &PreparedInstance) -> io::Result<()> {
    let servers = prepared.game_dir.join("servers.dat");
    let changed = match (modified(&servers), prepared.servers_snapshot) {
        (Some(now), Some(before)) => now > before,
        (Some(_), None) => true,
        (None, _) => false,
    };
    if changed {
        write_atomic(&paths.shared_gamedata().join("servers.dat"), &fs::read(&servers)?)?;
    }
    Ok(())
}

/// The most recently changed options.txt of any instance, used as template for new ones.
pub fn newest_options(paths: &AppPaths) -> Option<PathBuf> {
    fs::read_dir(paths.instances())
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("options.txt"))
        .filter_map(|p| modified(&p).map(|t| (t, p)))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    fn setup() -> (tempfile::TempDir, AppPaths) {
        let dir = tempfile::tempdir().unwrap();
        let p = AppPaths { home: dir.path().to_path_buf() };
        p.ensure_dirs().unwrap();
        (dir, p)
    }

    fn touch_later(path: &Path, content: &str) {
        std::thread::sleep(Duration::from_millis(30));
        fs::write(path, content).unwrap();
        let later = SystemTime::now() + Duration::from_secs(2);
        fs::File::options().write(true).open(path).unwrap().set_modified(later).unwrap();
    }

    #[test]
    fn shared_dirs_are_linked() {
        let (_d, p) = setup();
        prepare_instance(&p, "a", &[], None, None).unwrap();
        fs::create_dir_all(p.shared_gamedata().join("resourcepacks")).unwrap();
        fs::write(p.shared_gamedata().join("resourcepacks/pack.zip"), "x").unwrap();
        assert!(p.instance("a").join("resourcepacks/pack.zip").is_file());
        for d in SHARED_DIRS {
            assert!(p.instance("a").join(d).exists(), "{d}");
        }
    }

    #[test]
    fn mods_folder_mirrors_enabled_jars() {
        let (_d, p) = setup();
        let old = p.mods().join("old.jar");
        let new = p.mods().join("new.jar");
        let frog = p.home.join("frogclient.jar");
        fs::write(&old, "o").unwrap();
        fs::write(&new, "n").unwrap();
        fs::write(&frog, "f").unwrap();
        prepare_instance(&p, "a", &[old.clone()], None, None).unwrap();
        prepare_instance(&p, "a", &[new.clone()], Some(&frog), None).unwrap();
        let mods = p.instance("a").join("mods");
        assert!(!mods.join("old.jar").exists());
        assert_eq!(fs::read_to_string(mods.join("new.jar")).unwrap(), "n");
        assert!(mods.join("frogclient.jar").is_file());
    }

    #[test]
    fn existing_real_folder_is_migrated() {
        let (_d, p) = setup();
        let shots = p.instance("a").join("screenshots");
        fs::create_dir_all(&shots).unwrap();
        fs::write(shots.join("s.png"), "png").unwrap();
        prepare_instance(&p, "a", &[], None, None).unwrap();
        assert!(p.shared_gamedata().join("screenshots/s.png").is_file());
        assert!(shots.join("s.png").is_file());
    }

    #[test]
    fn servers_copied_in_and_back_when_changed() {
        let (_d, p) = setup();
        let shared = p.shared_gamedata().join("servers.dat");
        fs::write(&shared, "v1").unwrap();
        let prep = prepare_instance(&p, "a", &[], None, None).unwrap();
        let mine = prep.game_dir.join("servers.dat");
        assert_eq!(fs::read_to_string(&mine).unwrap(), "v1");
        touch_later(&mine, "v2-from-a");
        finish_instance(&p, &prep).unwrap();
        assert_eq!(fs::read_to_string(&shared).unwrap(), "v2-from-a");
    }

    #[test]
    fn servers_not_written_back_when_unchanged() {
        let (_d, p) = setup();
        let shared = p.shared_gamedata().join("servers.dat");
        fs::write(&shared, "v1").unwrap();
        let a = prepare_instance(&p, "a", &[], None, None).unwrap();
        let b = prepare_instance(&p, "b", &[], None, None).unwrap();
        touch_later(&b.game_dir.join("servers.dat"), "from-b");
        finish_instance(&p, &b).unwrap();
        finish_instance(&p, &a).unwrap(); // a changed nothing
        assert_eq!(fs::read_to_string(&shared).unwrap(), "from-b");
    }

    #[test]
    fn options_only_copied_when_missing() {
        let (_d, p) = setup();
        let tpl = p.home.join("tpl.txt");
        fs::write(&tpl, "fov:90").unwrap();
        prepare_instance(&p, "a", &[], None, Some(&tpl)).unwrap();
        let opts = p.instance("a").join("options.txt");
        assert_eq!(fs::read_to_string(&opts).unwrap(), "fov:90");
        fs::write(&opts, "fov:70").unwrap();
        prepare_instance(&p, "a", &[], None, Some(&tpl)).unwrap();
        assert_eq!(fs::read_to_string(&opts).unwrap(), "fov:70");
    }

    #[test]
    fn newest_options_picks_latest_mtime() {
        let (_d, p) = setup();
        for id in ["a", "b"] {
            fs::create_dir_all(p.instance(id)).unwrap();
            fs::write(p.instance(id).join("options.txt"), id).unwrap();
        }
        touch_later(&p.instance("a").join("options.txt"), "a2");
        assert_eq!(newest_options(&p).unwrap(), p.instance("a").join("options.txt"));
    }

    #[test]
    fn newest_options_none_without_instances() {
        let (_d, p) = setup();
        assert!(newest_options(&p).is_none());
    }
}
