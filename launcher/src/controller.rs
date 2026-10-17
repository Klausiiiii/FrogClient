use crate::accounts::Account;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRow {
    pub id: String,
    pub name: String,
    pub offline: bool,
    pub selected: bool,
    pub running: bool,
    pub error: Option<String>,
}

/// Label of the big start button; `None` disables it.
pub fn start_button_label(rows: &[AccountRow]) -> Option<String> {
    match rows.iter().filter(|r| r.selected && !r.running).count() {
        0 => None,
        1 => Some("► 1 Instanz starten".into()),
        n => Some(format!("► {n} Instanzen starten")),
    }
}

/// What a game process needs to know about the signed-in player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub xuid: String,
    pub user_type: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LaunchUpdate {
    Status(String),
    Progress(Option<f32>),
    AccountError { id: String, message: String },
    Started { id: String },
}

/// The steps of a launch, behind a trait so the orchestration is testable without
/// network, Java or Minecraft.
pub trait Backend: Send + Sync {
    type Prepared: Send + Sync;
    fn session(&self, account: &Account) -> impl Future<Output = Result<Session, String>> + Send;
    fn prepare(&self, report: &(dyn Fn(LaunchUpdate) + Send + Sync)) -> impl Future<Output = Result<Self::Prepared, String>> + Send;
    fn spawn(&self, account: &Account, session: Session, prepared: &Self::Prepared) -> impl Future<Output = Result<(), String>> + Send;
    /// Pause between two instance starts. Mods create shared config files on first start;
    /// starting everything at once races on them.
    fn stagger(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }
}

/// Signs in every account, installs the game once, then starts each instance. A failing
/// account is reported on its own row and never blocks the others.
pub async fn launch_accounts<B: Backend>(backend: &B, accounts: Vec<Account>, report: &(dyn Fn(LaunchUpdate) + Send + Sync)) {
    let mut ready = Vec::new();
    for account in accounts {
        report(LaunchUpdate::Status(format!("Melde {} an …", account.name)));
        match backend.session(&account).await {
            Ok(session) => ready.push((account, session)),
            Err(message) => report(LaunchUpdate::AccountError { id: account.id.clone(), message }),
        }
    }
    if ready.is_empty() {
        report(LaunchUpdate::Status(String::new()));
        return;
    }

    let prepared = match backend.prepare(report).await {
        Ok(p) => p,
        Err(message) => {
            for (account, _) in &ready {
                report(LaunchUpdate::AccountError { id: account.id.clone(), message: message.clone() });
            }
            report(LaunchUpdate::Progress(None));
            report(LaunchUpdate::Status(String::new()));
            return;
        }
    };

    for (i, (account, session)) in ready.into_iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(backend.stagger()).await;
        }
        report(LaunchUpdate::Status(format!("Starte {} …", account.name)));
        match backend.spawn(&account, session, &prepared).await {
            Ok(()) => report(LaunchUpdate::Started { id: account.id.clone() }),
            Err(message) => report(LaunchUpdate::AccountError { id: account.id.clone(), message }),
        }
    }
    report(LaunchUpdate::Progress(None));
    report(LaunchUpdate::Status(String::new()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::AccountKind;
    use std::sync::Mutex;

    fn row(id: &str, selected: bool, running: bool) -> AccountRow {
        AccountRow { id: id.into(), name: id.into(), offline: true, selected, running, error: None }
    }

    #[test]
    fn label_counts_selected_not_running() {
        let rows = [row("a", true, false), row("b", true, true), row("c", true, false), row("d", false, false)];
        assert_eq!(start_button_label(&rows).as_deref(), Some("► 2 Instanzen starten"));
    }

    #[test]
    fn label_singular() {
        assert_eq!(start_button_label(&[row("a", true, false)]).as_deref(), Some("► 1 Instanz starten"));
    }

    #[test]
    fn label_none_disables() {
        assert_eq!(start_button_label(&[row("a", false, false), row("b", true, true)]), None);
    }

    struct Fake {
        spawned: Mutex<Vec<String>>,
        prepared: Mutex<u32>,
        times: Mutex<Vec<std::time::Instant>>,
    }

    impl Backend for Fake {
        type Prepared = ();
        fn stagger(&self) -> std::time::Duration {
            std::time::Duration::from_millis(60)
        }
        async fn session(&self, account: &Account) -> Result<Session, String> {
            match account.kind {
                AccountKind::Microsoft { .. } => Err(crate::auth::AuthError::AppNotApproved.user_message().to_string()),
                AccountKind::Offline => Ok(Session {
                    name: account.name.clone(),
                    uuid: account.id.clone(),
                    access_token: "0".into(),
                    xuid: "0".into(),
                    user_type: "legacy".into(),
                }),
            }
        }
        async fn prepare(&self, _report: &(dyn Fn(LaunchUpdate) + Send + Sync)) -> Result<(), String> {
            *self.prepared.lock().unwrap() += 1;
            Ok(())
        }
        async fn spawn(&self, account: &Account, _session: Session, _prepared: &()) -> Result<(), String> {
            self.spawned.lock().unwrap().push(account.id.clone());
            self.times.lock().unwrap().push(std::time::Instant::now());
            Ok(())
        }
    }

    #[tokio::test]
    async fn failing_account_does_not_block_others() {
        let fake = Fake { spawned: Mutex::default(), prepared: Mutex::default(), times: Mutex::default() };
        let a = Account { id: "a".into(), name: "A".into(), kind: AccountKind::Microsoft { owns_game: true } };
        let b = Account { id: "b".into(), name: "B".into(), kind: AccountKind::Offline };
        let c = Account { id: "c".into(), name: "C".into(), kind: AccountKind::Offline };
        let updates = Mutex::new(Vec::new());
        launch_accounts(&fake, vec![a, b, c], &|u| updates.lock().unwrap().push(u)).await;

        assert_eq!(*fake.spawned.lock().unwrap(), vec!["b".to_string(), "c".to_string()]);
        assert_eq!(*fake.prepared.lock().unwrap(), 1, "game installed once for all instances");
        let updates = updates.lock().unwrap();
        assert!(updates.iter().any(|u| matches!(u, LaunchUpdate::AccountError { id, message } if id == "a" && message.contains("Mojang"))));
        assert!(updates.contains(&LaunchUpdate::Started { id: "b".into() }));
    }

    #[tokio::test]
    async fn instance_starts_are_staggered() {
        let fake = Fake { spawned: Mutex::default(), prepared: Mutex::default(), times: Mutex::default() };
        let b = Account { id: "b".into(), name: "B".into(), kind: AccountKind::Offline };
        let c = Account { id: "c".into(), name: "C".into(), kind: AccountKind::Offline };
        launch_accounts(&fake, vec![b, c], &|_| {}).await;
        let t = fake.times.lock().unwrap();
        assert!(t[1] - t[0] >= std::time::Duration::from_millis(60), "{:?}", t[1] - t[0]);
    }

    #[tokio::test]
    async fn nothing_prepared_when_all_accounts_fail() {
        let fake = Fake { spawned: Mutex::default(), prepared: Mutex::default(), times: Mutex::default() };
        let a = Account { id: "a".into(), name: "A".into(), kind: AccountKind::Microsoft { owns_game: true } };
        launch_accounts(&fake, vec![a], &|_| {}).await;
        assert_eq!(*fake.prepared.lock().unwrap(), 0);
    }
}
