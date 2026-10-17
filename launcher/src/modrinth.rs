use crate::http::{Http, HttpError};
use serde::Deserialize;

pub const MODRINTH_API: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub downloads: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModVersion {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    pub files: Vec<ModFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl ModVersion {
    pub fn primary_file(&self) -> Option<&ModFile> {
        self.files.iter().find(|f| f.primary).or(self.files.first())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModFile {
    pub url: String,
    pub filename: String,
    pub hashes: FileHashes,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileHashes {
    pub sha1: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dependency {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Clone)]
pub struct Modrinth {
    http: Http,
    base: String,
}

impl Modrinth {
    pub fn new(http: Http, base: &str) -> Modrinth {
        Modrinth { http, base: base.trim_end_matches('/').to_string() }
    }

    pub fn http(&self) -> &Http {
        &self.http
    }

    pub async fn search(&self, query: &str, mc: &str, offset: u32) -> Result<Vec<SearchHit>, HttpError> {
        #[derive(Deserialize)]
        struct SearchResponse {
            hits: Vec<SearchHit>,
        }
        let facets = format!(r#"[["versions:{mc}"],["categories:fabric"],["project_type:mod"]]"#);
        let url = format!("{}/search", self.base);
        let resp = self.http.client().get(&url)
            .query(&[("query", query), ("facets", facets.as_str()), ("limit", "20"), ("offset", &offset.to_string())])
            .send().await?;
        Ok(check(resp, &url).await?.json::<SearchResponse>().await?.hits)
    }

    pub async fn project(&self, id: &str) -> Result<Project, HttpError> {
        self.http.get_json(&format!("{}/project/{id}", self.base)).await
    }

    pub async fn version(&self, id: &str) -> Result<ModVersion, HttpError> {
        self.http.get_json(&format!("{}/version/{id}", self.base)).await
    }

    /// Best Fabric version for `mc`: release before beta before alpha, newest first.
    pub async fn best_version(&self, project: &str, mc: &str) -> Result<Option<ModVersion>, HttpError> {
        let url = format!("{}/project/{project}/version", self.base);
        let game_versions = format!(r#"["{mc}"]"#);
        let resp = self.http.client().get(&url)
            .query(&[("loaders", r#"["fabric"]"#), ("game_versions", game_versions.as_str())])
            .send().await?;
        let versions: Vec<ModVersion> = check(resp, &url).await?.json().await?;
        let rank = |t: &str| match t {
            "release" => 0,
            "beta" => 1,
            _ => 2,
        };
        // ISO-8601 timestamps sort lexicographically.
        Ok(versions.into_iter().min_by(|a, b| {
            rank(&a.version_type).cmp(&rank(&b.version_type)).then_with(|| b.date_published.cmp(&a.date_published))
        }))
    }
}

async fn check(resp: reqwest::Response, url: &str) -> Result<reqwest::Response, HttpError> {
    if resp.status().is_success() {
        Ok(resp)
    } else {
        Err(HttpError::Status(resp.status().as_u16(), url.to_string()))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::{json, Value};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    pub fn version(id: &str, project: &str, kind: &str, date: &str, server: &str, sha1: &str, deps: Value) -> Value {
        json!({
            "id": id, "project_id": project, "version_number": format!("{id}-v"), "version_type": kind,
            "date_published": date,
            "files": [{"url": format!("{server}/files/{id}.jar"), "filename": format!("{id}.jar"), "hashes": {"sha1": sha1}, "primary": true}],
            "dependencies": deps
        })
    }

    #[tokio::test]
    async fn search_sends_26_3_fabric_facets() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/search")).and(query_param("query", "sodium"))
            .and(query_param("facets", r#"[["versions:26.3"],["categories:fabric"],["project_type:mod"]]"#))
            .and(query_param("limit", "20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"hits": [
                {"project_id": "AANobbMI", "slug": "sodium", "title": "Sodium", "description": "fast", "downloads": 5}
            ]})))
            .expect(1).mount(&server).await;
        let mr = Modrinth::new(Http::new(), &server.uri());
        let hits = mr.search("sodium", "26.3", 0).await.unwrap();
        assert_eq!(hits[0].title, "Sodium");
    }

    #[tokio::test]
    async fn best_version_prefers_release() {
        let server = MockServer::start().await;
        let u = server.uri();
        Mock::given(method("GET")).and(path("/project/p/version"))
            .and(query_param("loaders", r#"["fabric"]"#)).and(query_param("game_versions", r#"["26.3"]"#))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                version("beta-new", "p", "beta", "2026-10-10T00:00:00Z", &u, "x", json!([])),
                version("rel-old", "p", "release", "2026-09-01T00:00:00Z", &u, "x", json!([])),
                version("rel-new", "p", "release", "2026-09-20T00:00:00Z", &u, "x", json!([])),
            ])))
            .mount(&server).await;
        let mr = Modrinth::new(Http::new(), &u);
        assert_eq!(mr.best_version("p", "26.3").await.unwrap().unwrap().id, "rel-new");
    }

    #[tokio::test]
    async fn best_version_none_when_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/project/p/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([]))).mount(&server).await;
        let mr = Modrinth::new(Http::new(), &server.uri());
        assert!(mr.best_version("p", "26.3").await.unwrap().is_none());
    }
}
