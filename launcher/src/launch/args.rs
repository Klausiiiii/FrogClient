use crate::fabric::LaunchProfile;
use crate::mojang::model::Arg;
use crate::mojang::rules::{rules_allow, Env, Features};
use crate::platform::CLASSPATH_SEP;
use std::collections::HashMap;
use std::path::PathBuf;

pub const AZURE_CLIENT_ID: &str = "2eb1f5a4-f452-4b42-acd4-880340c33f4c";

#[derive(Debug, Clone)]
pub struct LaunchContext {
    pub java: PathBuf,
    pub game_dir: PathBuf,
    pub assets_root: PathBuf,
    pub natives_dir: PathBuf,
    pub heap_mb: u32,
    pub extra_jvm: Vec<String>,
    pub player_name: String,
    /// Without dashes.
    pub uuid: String,
    pub access_token: String,
    pub xuid: String,
    /// "msa" or "legacy".
    pub user_type: String,
    pub config_dir: PathBuf,
    pub instance_name: String,
}

/// Full command line, java executable first.
pub fn build_command(p: &LaunchProfile, c: &LaunchContext, env: &Env) -> Vec<String> {
    let classpath = p
        .classpath
        .iter()
        .map(|x| x.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(CLASSPATH_SEP);
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", c.player_name.clone()),
        ("version_name", p.json.id.clone()),
        ("game_directory", c.game_dir.to_string_lossy().into_owned()),
        ("assets_root", c.assets_root.to_string_lossy().into_owned()),
        ("assets_index_name", p.json.asset_index.id.clone()),
        ("auth_uuid", c.uuid.clone()),
        ("auth_access_token", c.access_token.clone()),
        ("clientid", AZURE_CLIENT_ID.to_string()),
        ("auth_xuid", c.xuid.clone()),
        ("user_type", c.user_type.clone()),
        ("version_type", p.json.version_type.clone()),
        ("natives_directory", c.natives_dir.to_string_lossy().into_owned()),
        ("launcher_name", "FrogClient".to_string()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_string()),
        ("classpath", classpath),
    ]);

    let mut cmd = vec![
        c.java.to_string_lossy().into_owned(),
        format!("-Xmx{}M", c.heap_mb),
        "-Xms512M".to_string(),
    ];
    cmd.extend(resolve(&p.json.arguments.jvm, env, &vars));
    cmd.extend(p.extra_jvm_args.iter().cloned());
    if let (Some(config), Some(logging)) = (&p.logging_config, p.json.logging.as_ref().and_then(|l| l.client.as_ref())) {
        cmd.push(logging.argument.replace("${path}", &config.to_string_lossy()));
    }
    cmd.push(format!("-Dfrogclient.configDir={}", c.config_dir.to_string_lossy()));
    cmd.push(format!("-Dfrogclient.instanceName={}", c.instance_name));
    cmd.extend(c.extra_jvm.iter().cloned());
    cmd.push(p.main_class.clone());
    cmd.extend(resolve(&p.json.arguments.game, env, &vars));
    cmd.extend(p.extra_game_args.iter().cloned());
    cmd
}

fn resolve(args: &[Arg], env: &Env, vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Arg::Plain(s) => out.push(substitute(s, vars)),
            Arg::Conditional { rules, value } if rules_allow(rules, env, &Features) => {
                out.extend(value.values().into_iter().map(|s| substitute(s, vars)));
            }
            Arg::Conditional { .. } => {}
        }
    }
    out
}

fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let Some(len) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let key = &rest[start + 2..start + len];
        match vars.get(key) {
            Some(v) => out.push_str(v),
            None => {
                eprintln!("unknown launch placeholder ${{{key}}}");
                out.push_str(&rest[start..start + len + 1]);
            }
        }
        rest = &rest[start + len + 1..];
    }
    out.push_str(rest);
    out
}

/// Copy of the command safe for logs: the access token is masked.
pub fn redact(cmd: &[String]) -> Vec<String> {
    let mut out = cmd.to_vec();
    for i in 0..out.len().saturating_sub(1) {
        if out[i] == "--accessToken" {
            out[i + 1] = "***".into();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::{merge, FabricProfile};
    use crate::mojang::install::InstalledVersion;
    use crate::paths::AppPaths;

    const WIN64: Env = Env { os: "windows", arch: "x86_64" };

    fn profile() -> LaunchProfile {
        let p = AppPaths { home: PathBuf::from("H") };
        let json = serde_json::from_str(include_str!("../../tests/fixtures/26.3.json")).unwrap();
        let v = InstalledVersion {
            json,
            client_jar: p.versions().join("26.3/26.3.jar"),
            library_paths: vec![p.libraries().join("a/b/1/b-1.jar")],
            logging_config: Some(p.assets().join("log_configs/client-1.21.2.xml")),
        };
        let f: FabricProfile = serde_json::from_str(include_str!("../../tests/fixtures/fabric-profile.json")).unwrap();
        merge(&p, &v, &f, vec![p.libraries().join("net/fabricmc/x/1/x-1.jar")])
    }

    fn ctx() -> LaunchContext {
        LaunchContext {
            java: PathBuf::from("java.exe"),
            game_dir: PathBuf::from("G"),
            assets_root: PathBuf::from("A"),
            natives_dir: PathBuf::from("N"),
            heap_mb: 3072,
            extra_jvm: vec!["-Dextra=1".into()],
            player_name: "FrogMain".into(),
            uuid: "b50ad385829d3141a2167e7d7539ba7f".into(),
            access_token: "SECRET-TOKEN".into(),
            xuid: "0".into(),
            user_type: "msa".into(),
            config_dir: PathBuf::from("C"),
            instance_name: "FrogMain".into(),
        }
    }

    fn pos(cmd: &[String], s: &str) -> usize {
        cmd.iter().position(|a| a == s).unwrap_or_else(|| panic!("{s} missing in {cmd:?}"))
    }

    #[test]
    fn heap_and_main_class_present() {
        let cmd = build_command(&profile(), &ctx(), &WIN64);
        assert_eq!(cmd[0], "java.exe");
        assert!(pos(&cmd, "-Xmx3072M") < pos(&cmd, "net.fabricmc.loader.impl.launch.knot.KnotClient"));
    }

    #[test]
    fn placeholders_replaced() {
        let cmd = build_command(&profile(), &ctx(), &WIN64);
        assert!(!cmd.iter().any(|a| a.contains("${")), "{cmd:?}");
        assert_eq!(cmd[pos(&cmd, "--username") + 1], "FrogMain");
        assert_eq!(cmd[pos(&cmd, "--accessToken") + 1], "SECRET-TOKEN");
        assert_eq!(cmd[pos(&cmd, "--clientId") + 1], AZURE_CLIENT_ID);
        assert_eq!(cmd[pos(&cmd, "--version") + 1], "26.3");
    }

    #[test]
    fn classpath_uses_platform_separator() {
        let cmd = build_command(&profile(), &ctx(), &WIN64);
        let cp = &cmd[pos(&cmd, "-cp") + 1];
        assert!(cp.contains(crate::platform::CLASSPATH_SEP));
        assert!(cp.ends_with("26.3.jar"));
    }

    #[test]
    fn frogclient_properties_set() {
        let cmd = build_command(&profile(), &ctx(), &WIN64);
        assert!(cmd.contains(&"-Dfrogclient.configDir=C".to_string()));
        assert!(cmd.contains(&"-Dfrogclient.instanceName=FrogMain".to_string()));
        assert!(cmd.contains(&"-Dextra=1".to_string()));
        assert!(cmd.iter().any(|a| a.starts_with("-Dlog4j.configurationFile=") && a.ends_with("client-1.21.2.xml")));
        assert!(cmd.iter().any(|a| a.starts_with("-DFabricMcEmu=")));
    }

    #[test]
    fn no_demo_flag() {
        assert!(!build_command(&profile(), &ctx(), &WIN64).contains(&"--demo".to_string()));
    }

    #[test]
    fn redact_hides_token() {
        let cmd = build_command(&profile(), &ctx(), &WIN64);
        assert!(!redact(&cmd).iter().any(|a| a.contains("SECRET-TOKEN")));
    }
}
