use super::model::{Library, Rule, RuleAction, VersionJson};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Env {
    pub os: &'static str,
    pub arch: &'static str,
}

impl Env {
    pub fn current() -> Env {
        let os = match std::env::consts::OS {
            "macos" => "osx",
            "windows" => "windows",
            _ => "linux",
        };
        let arch = match std::env::consts::ARCH {
            "x86" => "x86",
            "aarch64" => "arm64",
            _ => "x86_64",
        };
        Env { os, arch }
    }
}

/// Launcher features referenced by argument rules. FrogClient enables none of them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Features;

impl Features {
    fn get(&self, _name: &str) -> bool {
        false
    }
}

/// Mojang semantics: no rules = allowed; otherwise start disallowed and let the last
/// matching rule decide.
pub fn rules_allow(rules: &[Rule], env: &Env, features: &Features) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, env, features) {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, env: &Env, features: &Features) -> bool {
    if let Some(os) = &rule.os {
        if os.name.as_deref().is_some_and(|n| n != env.os) {
            return false;
        }
        if os.arch.as_deref().is_some_and(|a| a != env.arch) {
            return false;
        }
    }
    if let Some(required) = &rule.features {
        if required.iter().any(|(name, value)| features.get(name) != *value) {
            return false;
        }
    }
    true
}

pub fn select_libraries<'a>(v: &'a VersionJson, env: &Env) -> Vec<&'a Library> {
    v.libraries
        .iter()
        .filter(|l| l.rules.as_deref().is_none_or(|r| rules_allow(r, env, &Features)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mojang::model::{Arg, VersionJson};

    const WIN64: Env = Env { os: "windows", arch: "x86_64" };
    const WIN32: Env = Env { os: "windows", arch: "x86" };

    fn fixture() -> VersionJson {
        serde_json::from_str(include_str!("../../tests/fixtures/26.3.json")).unwrap()
    }

    fn jvm_values(v: &VersionJson, env: &Env) -> Vec<String> {
        v.arguments.jvm.iter().flat_map(|a| match a {
            Arg::Plain(s) => vec![s.clone()],
            Arg::Conditional { rules, value } if rules_allow(rules, env, &Features) =>
                value.values().into_iter().map(String::from).collect(),
            Arg::Conditional { .. } => vec![],
        }).collect()
    }

    #[test]
    fn parses_26_3_fixture() {
        let v = fixture();
        assert_eq!(v.id, "26.3");
        assert_eq!(v.main_class, "net.minecraft.client.main.Main");
        assert_eq!(v.java_version.component, "java-runtime-epsilon");
        assert_eq!(v.java_version.major_version, 25);
    }

    #[test]
    fn windows_x64_skips_osx_and_linux_natives() {
        let v = fixture();
        let names: Vec<&str> = select_libraries(&v, &WIN64).iter().map(|l| l.name.as_str()).collect();
        assert!(names.iter().any(|n| n.ends_with("natives-windows")));
        assert!(!names.iter().any(|n| n.contains("natives-macos") || n.contains("natives-linux")));
        assert!(!names.iter().any(|n| n.contains("java-objc-bridge")));
        assert!(names.iter().any(|n| n.starts_with("at.yawk.lz4:lz4-java")));
    }

    #[test]
    fn x86_rule_only_on_32bit() {
        let v = fixture();
        assert!(!jvm_values(&v, &WIN64).contains(&"-Xss1M".to_string()));
        assert!(jvm_values(&v, &WIN32).contains(&"-Xss1M".to_string()));
    }

    #[test]
    fn feature_rules_off_by_default() {
        let v = fixture();
        let game: Vec<String> = v.arguments.game.iter().flat_map(|a| match a {
            Arg::Plain(s) => vec![s.clone()],
            Arg::Conditional { rules, value } if rules_allow(rules, &WIN64, &Features) =>
                value.values().into_iter().map(String::from).collect(),
            Arg::Conditional { .. } => vec![],
        }).collect();
        assert!(!game.contains(&"--demo".to_string()));
        assert!(game.contains(&"--username".to_string()));
    }

    #[test]
    fn no_rules_means_allow() {
        assert!(rules_allow(&[], &WIN64, &Features));
    }
}
