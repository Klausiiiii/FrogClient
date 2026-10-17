use futures::StreamExt;
use serde::de::DeserializeOwned;
use sha1::{Digest, Sha1};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

const ATTEMPTS: u32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("Netzwerkfehler: {0}")]
    Network(String),
    #[error("HTTP {0} bei {1}")]
    Status(u16, String),
    #[error("Prüfsumme stimmt nicht: {url}")]
    Hash { url: String },
    #[error("Dateifehler: {0}")]
    Io(String),
}

impl From<reqwest::Error> for HttpError {
    fn from(e: reqwest::Error) -> Self {
        HttpError::Network(e.to_string())
    }
}

impl From<io::Error> for HttpError {
    fn from(e: io::Error) -> Self {
        HttpError::Io(e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub bytes: u64,
}

#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
}

impl Default for Http {
    fn default() -> Self {
        Http::new()
    }
}

impl Http {
    pub fn new() -> Http {
        let client = reqwest::Client::builder()
            .user_agent(concat!("FrogClient-Launcher/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .build()
            .expect("HTTP client");
        Http { client }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, HttpError> {
        let resp = self.client.get(url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(HttpError::Status(status.as_u16(), url.to_string()));
        }
        Ok(resp.json::<T>().await?)
    }

    /// Downloads all jobs with at most `parallel` in flight. Files that already exist with
    /// the expected hash are skipped; everything else goes to `<dest>.part`, is verified and
    /// only then renamed into place, so a half-written file is never taken as valid.
    pub async fn download_all(
        &self,
        jobs: Vec<DownloadJob>,
        parallel: usize,
        progress: impl Fn(Progress) + Send + Sync,
    ) -> Result<(), HttpError> {
        let total = jobs.len();
        let done = AtomicUsize::new(0);
        let bytes = AtomicU64::new(0);
        let progress = &progress;
        let (done, bytes) = (&done, &bytes);

        let mut results = futures::stream::iter(jobs)
            .map(|job| async move {
                let n = self.download_one(&job).await?;
                let b = bytes.fetch_add(n, Ordering::Relaxed) + n;
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                progress(Progress { done: d, total, bytes: b });
                Ok::<(), HttpError>(())
            })
            .buffer_unordered(parallel.max(1));

        while let Some(r) = results.next().await {
            r?;
        }
        Ok(())
    }

    async fn download_one(&self, job: &DownloadJob) -> Result<u64, HttpError> {
        if let Some(expected) = &job.sha1 {
            if job.dest.is_file() && file_matches(&job.dest, expected).await {
                return Ok(0);
            }
        } else if job.dest.is_file() {
            return Ok(0);
        }

        let mut last = HttpError::Network("no attempt".into());
        for attempt in 0..ATTEMPTS {
            match self.fetch_to_file(job).await {
                Ok(n) => return Ok(n),
                // A wrong hash will not fix itself by retrying.
                Err(e @ HttpError::Hash { .. }) => return Err(e),
                Err(e) => last = e,
            }
            tokio::time::sleep(Duration::from_millis(200 * (attempt as u64 + 1))).await;
        }
        Err(last)
    }

    async fn fetch_to_file(&self, job: &DownloadJob) -> Result<u64, HttpError> {
        if let Some(parent) = job.dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let part = part_path(&job.dest);
        let result = async {
            let resp = self.client.get(&job.url).send().await?;
            let status = resp.status();
            if !status.is_success() {
                return Err(HttpError::Status(status.as_u16(), job.url.clone()));
            }
            let mut file = tokio::fs::File::create(&part).await?;
            let mut hasher = Sha1::new();
            let mut written = 0u64;
            let mut stream = resp.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                hasher.update(&chunk);
                file.write_all(&chunk).await?;
                written += chunk.len() as u64;
            }
            file.flush().await?;
            drop(file);
            if let Some(expected) = &job.sha1 {
                if !hex::encode(hasher.finalize()).eq_ignore_ascii_case(expected) {
                    return Err(HttpError::Hash { url: job.url.clone() });
                }
            }
            tokio::fs::rename(&part, &job.dest).await?;
            Ok(written)
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&part).await;
        }
        result
    }
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

async fn file_matches(path: &Path, expected: &str) -> bool {
    let path = path.to_path_buf();
    let expected = expected.to_string();
    tokio::task::spawn_blocking(move || sha1_file(&path).map(|h| h.eq_ignore_ascii_case(&expected)).unwrap_or(false))
        .await
        .unwrap_or(false)
}

pub fn sha1_file(path: &Path) -> io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // sha1("hello")
    const HELLO_SHA1: &str = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";

    fn job(server: &MockServer, name: &str, dest: &Path) -> DownloadJob {
        DownloadJob {
            url: format!("{}/{}", server.uri(), name),
            dest: dest.to_path_buf(),
            sha1: Some(HELLO_SHA1.into()),
            size: Some(5),
        }
    }

    #[tokio::test]
    async fn downloads_and_verifies() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/a"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello"))
            .mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("sub/a.bin");
        Http::new().download_all(vec![job(&server, "a", &dest)], 4, |_| {}).await.unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hello");
        assert!(!dir.path().join("sub/a.bin.part").exists());
    }

    #[tokio::test]
    async fn skips_existing_valid_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_string("hello"))
            .expect(0).mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("a.bin");
        std::fs::write(&dest, "hello").unwrap();
        Http::new().download_all(vec![job(&server, "a", &dest)], 4, |_| {}).await.unwrap();
    }

    #[tokio::test]
    async fn replaces_corrupt_existing_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/a"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello"))
            .expect(1).mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("a.bin");
        std::fs::write(&dest, "kaputt").unwrap();
        Http::new().download_all(vec![job(&server, "a", &dest)], 4, |_| {}).await.unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hello");
    }

    #[tokio::test]
    async fn hash_mismatch_fails_and_leaves_no_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/a"))
            .respond_with(ResponseTemplate::new(200).set_body_string("evil!"))
            .mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("a.bin");
        let err = Http::new().download_all(vec![job(&server, "a", &dest)], 4, |_| {}).await.unwrap_err();
        assert!(matches!(err, HttpError::Hash { .. }), "{err:?}");
        assert!(!dest.exists());
        assert!(!dir.path().join("a.bin.part").exists());
    }

    #[tokio::test]
    async fn retries_then_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/a"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(2).mount(&server).await;
        Mock::given(method("GET")).and(path("/a"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello"))
            .mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("a.bin");
        Http::new().download_all(vec![job(&server, "a", &dest)], 4, |_| {}).await.unwrap();
        assert!(dest.exists());
    }

    #[tokio::test]
    async fn reports_progress_total() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(200).set_body_string("hello"))
            .mount(&server).await;
        let dir = tempfile::tempdir().unwrap();
        let jobs = (0..3).map(|i| job(&server, &format!("f{i}"), &dir.path().join(format!("f{i}")))).collect();
        let last = Mutex::new(None);
        Http::new().download_all(jobs, 2, |p| *last.lock().unwrap() = Some(p)).await.unwrap();
        let p = last.lock().unwrap().unwrap();
        assert_eq!((p.done, p.total), (3, 3));
    }

    #[tokio::test]
    async fn get_json_reports_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).respond_with(ResponseTemplate::new(404)).mount(&server).await;
        let err = Http::new().get_json::<serde_json::Value>(&server.uri()).await.unwrap_err();
        assert!(matches!(err, HttpError::Status(404, _)));
    }

    #[test]
    fn sha1_of_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x");
        std::fs::write(&f, "hello").unwrap();
        assert_eq!(sha1_file(&f).unwrap(), HELLO_SHA1);
    }
}
