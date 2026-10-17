//! Three-way merge of Minecraft's `servers.dat` (uncompressed NBT: `{servers: [{ip, name, icon?, ...}]}`).
//!
//! Minecraft rewrites the file whenever it joins a server or refreshes an icon, so a
//! changed file says nothing about whether the player changed the list. Only real edits
//! (added, removed or edited servers, keyed by address) are carried into the shared list.

use fastnbt::Value;
use std::collections::HashMap;

type Root = HashMap<String, Value>;

fn parse(bytes: &[u8]) -> Option<(Root, Vec<Value>)> {
    if bytes.is_empty() {
        return Some((Root::new(), Vec::new()));
    }
    let Value::Compound(mut root) = fastnbt::from_bytes::<Value>(bytes).ok()? else { return None };
    let servers = match root.remove("servers") {
        Some(Value::List(list)) => list,
        None => Vec::new(),
        Some(_) => return None,
    };
    Some((root, servers))
}

fn key(server: &Value) -> Option<String> {
    let Value::Compound(c) = server else { return None };
    match c.get("ip") {
        Some(Value::String(ip)) => Some(ip.trim().to_lowercase()),
        _ => None,
    }
}

/// Server fields that express a player's edit; the cached icon is excluded.
fn edit_fields(server: &Value) -> Option<HashMap<&String, &Value>> {
    let Value::Compound(c) = server else { return None };
    Some(c.iter().filter(|(k, _)| k.as_str() != "icon").collect())
}

fn by_key(list: &[Value]) -> HashMap<String, &Value> {
    list.iter().filter_map(|s| key(s).map(|k| (k, s))).collect()
}

/// Merges this instance's edits (`base` → `mine`) into the current shared list.
/// Returns `None` when the instance made no real change or a file cannot be read;
/// the shared file must then be left alone. Empty slices mean "file did not exist".
pub fn merge(base: &[u8], mine: &[u8], shared: &[u8]) -> Option<Vec<u8>> {
    let (_, base_list) = parse(base)?;
    let (_, mine_list) = parse(mine)?;
    let (mut root, mut result) = parse(shared)?;
    let base_map = by_key(&base_list);
    let mine_map = by_key(&mine_list);

    let removed: Vec<&String> = base_map.keys().filter(|k| !mine_map.contains_key(*k)).collect();
    let added: Vec<&Value> = mine_list.iter().filter(|s| key(s).is_some_and(|k| !base_map.contains_key(&k))).collect();
    let edited: Vec<(String, &Value)> = mine_map
        .iter()
        .filter(|(k, s)| base_map.get(*k).is_some_and(|b| edit_fields(b) != edit_fields(s)))
        .map(|(k, s)| (k.clone(), *s))
        .collect();
    if removed.is_empty() && added.is_empty() && edited.is_empty() {
        return None;
    }

    result.retain(|s| key(s).is_none_or(|k| !removed.contains(&&k)));
    for (k, s) in &edited {
        match result.iter_mut().find(|r| key(r).as_ref() == Some(k)) {
            Some(slot) => *slot = (*s).clone(),
            None => result.push((*s).clone()),
        }
    }
    for s in added {
        if !result.iter().any(|r| key(r) == key(s)) {
            result.push(s.clone());
        }
    }

    root.insert("servers".into(), Value::List(result));
    fastnbt::to_bytes(&Value::Compound(root)).ok()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use fastnbt::Value;
    use std::collections::HashMap;

    pub fn server(ip: &str, name: &str, icon: Option<&str>) -> Value {
        let mut c = HashMap::from([
            ("ip".to_string(), Value::String(ip.into())),
            ("name".to_string(), Value::String(name.into())),
        ]);
        if let Some(icon) = icon {
            c.insert("icon".into(), Value::String(icon.into()));
        }
        Value::Compound(c)
    }

    pub fn file(servers: Vec<Value>) -> Vec<u8> {
        fastnbt::to_bytes(&Value::Compound(HashMap::from([("servers".to_string(), Value::List(servers))]))).unwrap()
    }

    fn entries(bytes: &[u8]) -> Vec<(String, String)> {
        let Value::Compound(root) = fastnbt::from_bytes::<Value>(bytes).unwrap() else { panic!() };
        let Some(Value::List(list)) = root.get("servers") else { panic!() };
        list.iter()
            .map(|s| {
                let Value::Compound(c) = s else { panic!() };
                let get = |k: &str| match c.get(k) {
                    Some(Value::String(v)) => v.clone(),
                    _ => String::new(),
                };
                (get("ip"), get("name"))
            })
            .collect()
    }

    fn pair(ip: &str, name: &str) -> (String, String) {
        (ip.into(), name.into())
    }

    #[test]
    fn icon_refresh_alone_is_no_change() {
        let base = file(vec![server("y.net", "Y", Some("old"))]);
        let mine = file(vec![server("y.net", "Y", Some("new-icon"))]);
        let shared = file(vec![server("y.net", "Y", Some("old")), server("x.net", "X", None)]);
        assert_eq!(merge(&base, &mine, &shared), None, "an instance that only joined Y must not drop X");
    }

    #[test]
    fn my_addition_is_appended_to_shared() {
        let base = file(vec![server("y.net", "Y", None)]);
        let mine = file(vec![server("y.net", "Y", None), server("z.net", "Z", None)]);
        let shared = file(vec![server("y.net", "Y", None), server("x.net", "X", None)]);
        let out = merge(&base, &mine, &shared).unwrap();
        assert_eq!(entries(&out), vec![pair("y.net", "Y"), pair("x.net", "X"), pair("z.net", "Z")]);
    }

    #[test]
    fn my_removal_is_applied_but_others_additions_stay() {
        let base = file(vec![server("y.net", "Y", None), server("x.net", "X", None)]);
        let mine = file(vec![server("x.net", "X", None)]);
        let shared = file(vec![server("y.net", "Y", None), server("x.net", "X", None), server("w.net", "W", None)]);
        let out = merge(&base, &mine, &shared).unwrap();
        assert_eq!(entries(&out), vec![pair("x.net", "X"), pair("w.net", "W")]);
    }

    #[test]
    fn my_rename_wins() {
        let base = file(vec![server("y.net", "old", None)]);
        let mine = file(vec![server("y.net", "new", None)]);
        let shared = file(vec![server("y.net", "old", None)]);
        assert_eq!(entries(&merge(&base, &mine, &shared).unwrap()), vec![pair("y.net", "new")]);
    }

    #[test]
    fn first_list_ever_is_published() {
        let mine = file(vec![server("z.net", "Z", None)]);
        assert_eq!(entries(&merge(&[], &mine, &[]).unwrap()), vec![pair("z.net", "Z")]);
    }

    #[test]
    fn address_match_ignores_case_and_spaces() {
        let base = file(vec![server("Y.net ", "Y", None)]);
        let mine = file(vec![]);
        let shared = file(vec![server("y.net", "Y", None)]);
        assert_eq!(entries(&merge(&base, &mine, &shared).unwrap()), vec![]);
    }

    #[test]
    fn unreadable_file_changes_nothing() {
        let shared = file(vec![server("x.net", "X", None)]);
        assert_eq!(merge(&[], b"not nbt", &shared), None);
    }
}
