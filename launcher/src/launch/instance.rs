use crate::fsutil::write_atomic;
use crate::launch::servers;
use crate::paths::AppPaths;
use crate::platform::{link_dir, link_or_copy_file};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Folders every instance shares through a directory link (spec 6.2).
pub const SHARED_DIRS: [&str; 4] = ["resourcepacks", "shaderpacks", "screenshots", "config"];

/// Launcher bookkeeping inside an instance; survives a launcher restart.
const META_DIR: &str = ".frogclient";
const SERVERS_BASE: &str = "servers.base.dat";
const PID_FILE: &str = "pid";

#[derive(Debug, Clone)]
pub struct PreparedInstance {
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
}

fn meta(game_dir: &Path) -> PathBuf {
    game_dir.join(META_DIR)
}

/// Builds `instances/<id>/` for one launch: shared folders linked, mods mirrored,
/// server list copied in (with its base kept for the merge on exit), options.txt seeded.
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
    fs::create_dir_all(meta(&game_dir))?;
    // A previous session that was never finished (launcher closed, crash) is merged first.
    finish_instance(paths, &game_dir)?;

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

    // Minecraft rewrites servers.dat by replacing the file, which breaks hard links, so each
    // instance gets a copy; the copy's starting point is kept for the three-way merge.
    let shared = fs::read(paths.shared_gamedata().join("servers.dat")).unwrap_or_default();
    let servers = game_dir.join("servers.dat");
    if !shared.is_empty() {
        fs::write(&servers, &shared)?;
    }
    write_atomic(&meta(&game_dir).join(SERVERS_BASE), &shared)?;

    let options = game_dir.join("options.txt");
    if let Some(template) = options_template {
        if !options.exists() && template.is_file() {
            fs::copy(template, &options)?;
        }
    }

    Ok(PreparedInstance { game_dir, natives_dir })
}

/// After the game exits (or on the next launcher start): merge this session's server list
/// edits into the shared list. Does nothing when no session is pending.
pub fn finish_instance(paths: &AppPaths, game_dir: &Path) -> io::Result<()> {
    let base_path = meta(game_dir).join(SERVERS_BASE);
    let Ok(base) = fs::read(&base_path) else {
        clear_pid(game_dir);
        return Ok(());
    };
    let mine = fs::read(game_dir.join("servers.dat")).unwrap_or_default();
    let shared_path = paths.shared_gamedata().join("servers.dat");
    let shared = fs::read(&shared_path).unwrap_or_default();
    if let Some(merged) = servers::merge(&base, &mine, &shared) {
        write_atomic(&shared_path, &merged)?;
    }
    fs::remove_file(&base_path)?;
    clear_pid(game_dir);
    Ok(())
}

pub fn write_pid(game_dir: &Path, pid: u32) -> io::Result<()> {
    write_atomic(&meta(game_dir).join(PID_FILE), pid.to_string().as_bytes())
}

pub fn read_pid(game_dir: &Path) -> Option<u32> {
    fs::read_to_string(meta(game_dir).join(PID_FILE)).ok()?.trim().parse().ok()
}

pub fn clear_pid(game_dir: &Path) {
    let _ = fs::remove_file(meta(game_dir).join(PID_FILE));
}

/// Instances whose last session was never finished, with the game's pid if one was recorded.
pub fn pending_instances(paths: &AppPaths) -> Vec<(String, Option<u32>)> {
    let Ok(dir) = fs::read_dir(paths.instances()) else { return Vec::new() };
    let mut out: Vec<(String, Option<u32>)> = dir
        .filter_map(|e| e.ok())
        .filter(|e| meta(&e.path()).join(SERVERS_BASE).is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), read_pid(&e.path())))
        .collect();
    out.sort();
    out
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

    use crate::launch::servers::tests::{file, server};

    fn shared_ips(p: &AppPaths) -> Vec<String> {
        let bytes = fs::read(p.shared_gamedata().join("servers.dat")).unwrap();
        let fastnbt::Value::Compound(root) = fastnbt::from_bytes::<fastnbt::Value>(&bytes).unwrap() else { panic!() };
        let Some(fastnbt::Value::List(list)) = root.get("servers") else { panic!() };
        list.iter().map(|s| match s {
            fastnbt::Value::Compound(c) => match c.get("ip") { Some(fastnbt::Value::String(ip)) => ip.clone(), _ => String::new() },
            _ => String::new(),
        }).collect()
    }

    #[test]
    fn servers_copied_in_and_merged_back() {
        let (_d, p) = setup();
        fs::write(p.shared_gamedata().join("servers.dat"), file(vec![server("y.net", "Y", None)])).unwrap();
        let prep = prepare_instance(&p, "a", &[], None, None).unwrap();
        let mine = prep.game_dir.join("servers.dat");
        assert_eq!(fs::read(&mine).unwrap(), file(vec![server("y.net", "Y", None)]));
        fs::write(&mine, file(vec![server("y.net", "Y", None), server("z.net", "Z", None)])).unwrap();
        finish_instance(&p, &prep.game_dir).unwrap();
        assert_eq!(shared_ips(&p), vec!["y.net", "z.net"]);
    }

    #[test]
    fn instance_that_only_played_does_not_drop_others_servers() {
        let (_d, p) = setup();
        fs::write(p.shared_gamedata().join("servers.dat"), file(vec![server("y.net", "Y", None)])).unwrap();
        let a = prepare_instance(&p, "a", &[], None, None).unwrap();
        let b = prepare_instance(&p, "b", &[], None, None).unwrap();
        fs::write(b.game_dir.join("servers.dat"), file(vec![server("y.net", "Y", None), server("x.net", "X", None)])).unwrap();
        finish_instance(&p, &b.game_dir).unwrap();
        // a joined Y: Minecraft rewrote the file with a fresh icon, but the list is unchanged.
        fs::write(a.game_dir.join("servers.dat"), file(vec![server("y.net", "Y", Some("icon"))])).unwrap();
        finish_instance(&p, &a.game_dir).unwrap();
        assert_eq!(shared_ips(&p), vec!["y.net", "x.net"]);
    }

    #[test]
    fn merge_still_works_after_launcher_restart() {
        let (_d, p) = setup();
        fs::write(p.shared_gamedata().join("servers.dat"), file(vec![server("y.net", "Y", None)])).unwrap();
        let game_dir = prepare_instance(&p, "a", &[], None, None).unwrap().game_dir;
        fs::write(game_dir.join("servers.dat"), file(vec![])).unwrap();
        // Launcher closed and reopened: only the files on disk are left.
        finish_instance(&p, &game_dir).unwrap();
        assert!(shared_ips(&p).is_empty());
        // Finishing twice is harmless.
        finish_instance(&p, &game_dir).unwrap();
        assert!(shared_ips(&p).is_empty());
    }

    #[test]
    fn pid_file_roundtrip_and_stale_detection() {
        let (_d, p) = setup();
        let game_dir = prepare_instance(&p, "a", &[], None, None).unwrap().game_dir;
        assert_eq!(read_pid(&game_dir), None);
        write_pid(&game_dir, 4242).unwrap();
        assert_eq!(read_pid(&game_dir), Some(4242));
        clear_pid(&game_dir);
        assert_eq!(read_pid(&game_dir), None);
    }

    #[test]
    fn pending_instances_lists_unfinished_sessions() {
        let (_d, p) = setup();
        let a = prepare_instance(&p, "a", &[], None, None).unwrap().game_dir;
        write_pid(&a, 7).unwrap();
        let b = prepare_instance(&p, "b", &[], None, None).unwrap().game_dir;
        finish_instance(&p, &b).unwrap();
        assert_eq!(pending_instances(&p), vec![("a".to_string(), Some(7))]);
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
