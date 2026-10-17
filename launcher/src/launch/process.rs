use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

/// Extra memory per instance beyond the heap (metaspace, natives, GPU driver).
const OVERHEAD_MB: u64 = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceEvent {
    Started { account_id: String },
    Exited { account_id: String, code: Option<i32>, log: PathBuf },
}

/// Tracks one game process per account; events go to the controller.
#[derive(Clone)]
pub struct ProcessManager {
    events: mpsc::UnboundedSender<InstanceEvent>,
    running: Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
}

impl ProcessManager {
    pub fn new(events: mpsc::UnboundedSender<InstanceEvent>) -> Self {
        ProcessManager { events, running: Arc::default() }
    }

    /// Starts `cmd` in `cwd`. Output is discarded: Minecraft writes its own logs.
    /// `on_exit` runs after the process ended, before the `Exited` event is sent.
    pub async fn spawn(
        &self,
        account_id: String,
        cmd: Vec<String>,
        cwd: PathBuf,
        on_exit: impl FnOnce() + Send + 'static,
    ) -> io::Result<()> {
        let (program, args) = cmd.split_first().ok_or_else(|| io::Error::other("empty command"))?;
        let mut command = tokio::process::Command::new(program);
        command.args(args).current_dir(&cwd).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn()?;

        let (kill_tx, kill_rx) = oneshot::channel();
        self.running.lock().unwrap().insert(account_id.clone(), kill_tx);
        let _ = self.events.send(InstanceEvent::Started { account_id: account_id.clone() });

        let running = self.running.clone();
        let events = self.events.clone();
        tokio::spawn(async move {
            let status = tokio::select! {
                status = child.wait() => status,
                _ = kill_rx => {
                    let _ = child.kill().await;
                    child.wait().await
                }
            };
            running.lock().unwrap().remove(&account_id);
            on_exit();
            let code = status.ok().and_then(|s| s.code());
            let _ = events.send(InstanceEvent::Exited { account_id, code, log: cwd.join("logs").join("latest.log") });
        });
        Ok(())
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.running.lock().unwrap().contains_key(id)
    }

    pub fn kill(&self, id: &str) {
        if let Some(tx) = self.running.lock().unwrap().remove(id) {
            let _ = tx.send(());
        }
    }

    pub fn running(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.running.lock().unwrap().keys().cloned().collect();
        ids.sort();
        ids
    }
}

pub fn ram_warning(heap_mb_each: u32, count: usize, available_mb: u64) -> Option<String> {
    let needed = count as u64 * (heap_mb_each as u64 + OVERHEAD_MB);
    if needed <= available_mb {
        return None;
    }
    let gb = |mb: u64| format!("{:.1}", mb as f64 / 1024.0).replace('.', ",");
    let noun = if count == 1 { "Instanz" } else { "Instanzen" };
    Some(format!(
        "Für {count} {noun} werden etwa {} GB Arbeitsspeicher benötigt, frei sind nur {} GB.",
        gb(needed),
        gb(available_mb)
    ))
}

pub fn available_memory_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.available_memory() / (1024 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[test]
    fn ram_ok() {
        assert_eq!(ram_warning(3072, 2, 16000), None);
    }

    #[test]
    fn ram_too_low() {
        let w = ram_warning(3072, 4, 10000).unwrap();
        assert!(w.contains("4 Instanzen"), "{w}");
    }

    #[test]
    fn available_memory_is_plausible() {
        assert!(available_memory_mb() > 0);
    }

    async fn next_exit(rx: &mut mpsc::UnboundedReceiver<InstanceEvent>) -> InstanceEvent {
        loop {
            let ev = tokio::time::timeout(Duration::from_secs(20), rx.recv()).await.unwrap().unwrap();
            if matches!(ev, InstanceEvent::Exited { .. }) {
                return ev;
            }
        }
    }

    #[cfg(windows)]
    fn cmd(args: &[&str]) -> Vec<String> {
        ["cmd", "/C"].iter().chain(args).map(|s| s.to_string()).collect()
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn exit_code_reported() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pm = ProcessManager::new(tx);
        let dir = tempfile::tempdir().unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let c = called.clone();
        pm.spawn("a".into(), cmd(&["exit", "3"]), dir.path().into(), move || c.store(true, Ordering::SeqCst)).await.unwrap();
        let ev = next_exit(&mut rx).await;
        assert_eq!(ev, InstanceEvent::Exited { account_id: "a".into(), code: Some(3), log: dir.path().join("logs/latest.log") });
        assert!(!pm.is_running("a"));
        assert!(called.load(Ordering::SeqCst));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn kill_stops_process() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pm = ProcessManager::new(tx);
        let dir = tempfile::tempdir().unwrap();
        pm.spawn("a".into(), cmd(&["ping", "-n", "30", "127.0.0.1"]), dir.path().into(), || {}).await.unwrap();
        assert!(pm.is_running("a"));
        pm.kill("a");
        next_exit(&mut rx).await;
        assert!(!pm.is_running("a"));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn two_processes_independent() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pm = ProcessManager::new(tx);
        let dir = tempfile::tempdir().unwrap();
        pm.spawn("long".into(), cmd(&["ping", "-n", "30", "127.0.0.1"]), dir.path().into(), || {}).await.unwrap();
        pm.spawn("short".into(), cmd(&["exit", "0"]), dir.path().into(), || {}).await.unwrap();
        let ev = next_exit(&mut rx).await;
        assert!(matches!(ev, InstanceEvent::Exited { ref account_id, .. } if account_id == "short"));
        assert_eq!(pm.running(), vec!["long".to_string()]);
        pm.kill("long");
        next_exit(&mut rx).await;
    }

    #[tokio::test]
    async fn spawn_failure_is_error() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let pm = ProcessManager::new(tx);
        let dir = tempfile::tempdir().unwrap();
        assert!(pm.spawn("x".into(), vec!["definitely-not-a-program-xyz".into()], dir.path().into(), || {}).await.is_err());
        assert!(!pm.is_running("x"));
    }
}
