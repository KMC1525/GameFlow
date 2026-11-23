//! Servidor local: API REST + SSE para a interface e um segundo servidor, isolado (outra origem),
//! que executa os jogos gerados. Escuta apenas em 127.0.0.1.

use crate::agents::run::{Envelope, GenerateRequest, RunEvent};
use crate::agents::{self, Orchestrator};
use crate::config::{Paths, Settings};
use crate::hf::{self, Hf};
use crate::projects::{HIDDEN_DIR, META_FILE, Projects};
use crate::runtime::{Runtimes, Task};
use crate::spec::{Genre, theme_ids};
use crate::{templates, util};
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tokio_stream::wrappers::BroadcastStream;

#[derive(RustEmbed)]
#[folder = "frontend/"]
struct Frontend;

pub struct AppState {
    pub paths: Paths,
    pub settings: Arc<RwLock<Settings>>,
    pub projects: Projects,
    pub rt: Arc<Runtimes>,
    pub hf: Hf,
    pub orch: Arc<Orchestrator>,
    pub play_port: RwLock<u16>,
    pub port: RwLock<u16>,
}

pub type S = Arc<AppState>;

pub fn build_state(paths: Paths) -> anyhow::Result<S> {
    paths.ensure()?;
    let mut settings = Settings::load(&paths.settings_file);
    settings.sanitize();
    let settings = Arc::new(RwLock::new(settings));
    let projects = Projects::new(paths.projects.clone());
    let rt = Arc::new(Runtimes::new(paths.clone(), settings.clone()));
    let hf = Hf::new(paths.clone(), settings.clone());
    let orch = Orchestrator::new(projects.clone(), rt.clone(), settings.clone());
    Ok(Arc::new(AppState { paths, settings, projects, rt, hf, orch, play_port: RwLock::new(0), port: RwLock::new(0) }))
}

// ───────────────────────── erros ─────────────────────────

pub struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError(StatusCode::BAD_REQUEST, format!("{e:#}"))
    }
}
impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    }
}

fn not_found(msg: &str) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, msg.into())
}

type R<T> = Result<T, ApiError>;

// ───────────────────────── segurança ─────────────────────────

fn host_only(h: &str) -> &str {
    if let Some(rest) = h.strip_prefix('[') {
        return rest.split(']').next().unwrap_or("");
    }
    h.split(':').next().unwrap_or("")
}

/// Bloqueia DNS-rebinding (Host inesperado) e requisições vindas de outras origens (CSRF).
async fn guard(req: Request, next: Next) -> Response {
    let headers = req.headers();
    let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
    if !matches!(host_only(&host), "127.0.0.1" | "localhost" | "::1") {
        return (StatusCode::FORBIDDEN, "host não permitido").into_response();
    }
    if let Some(origin) = headers.get(header::ORIGIN).and_then(|o| o.to_str().ok()) {
        let o = origin.split("://").nth(1).unwrap_or(origin);
        if o != host {
            return (StatusCode::FORBIDDEN, "origem não permitida").into_response();
        }
    }
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.entry(header::CACHE_CONTROL).or_insert(HeaderValue::from_static("no-store"));
    resp
}

// ───────────────────────── rotas ─────────────────────────

pub fn router(st: S) -> Router {
    let api = Router::new()
        .route("/info", get(info))
        .route("/settings", get(get_settings).put(put_settings))
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/{id}", get(get_project).patch(patch_project).delete(delete_project))
        .route("/projects/{id}/tree", get(tree))
        .route("/projects/{id}/file", get(read_file).put(write_file).post(create_entry).delete(delete_entry))
        .route("/projects/{id}/move", post(move_entry))
        .route("/projects/{id}/upload", post(upload).layer(DefaultBodyLimit::max(256 * 1024 * 1024)))
        .route("/projects/{id}/export", get(export))
        .route("/projects/{id}/chat", get(chat))
        .route("/projects/{id}/generate", post(generate))
        .route("/projects/{id}/active-run", get(active_run))
        .route("/projects/{id}/open-folder", post(open_folder))
        .route("/runs/{run_id}/events", get(run_events))
        .route("/runs/{run_id}/cancel", post(run_cancel))
        .route("/hf/search", get(hf_search))
        .route("/hf/files", get(hf_files))
        .route("/hf/recommended", get(hf_recommended))
        .route("/hf/download", post(hf_download))
        .route("/downloads", get(downloads).delete(clear_downloads))
        .route("/downloads/cancel", post(cancel_download))
        .route("/models", get(local_models).delete(delete_model))
        .route("/agents/test", post(agent_test));
    Router::new().nest("/api", api).fallback(static_files).layer(middleware::from_fn(guard)).with_state(st)
}

async fn static_files(req: Request) -> Response {
    let path = req.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Frontend::get(path) {
        Some(f) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder().header(header::CONTENT_TYPE, mime.as_ref()).header(header::CACHE_CONTROL, "no-cache").body(Body::from(f.data.into_owned())).unwrap()
        }
        None => (StatusCode::NOT_FOUND, "não encontrado").into_response(),
    }
}

// ───────────────────────── info / configurações ─────────────────────────

async fn info(State(st): State<S>) -> Json<Value> {
    let genres: Vec<Value> = Genre::ALL.iter().map(|g| json!({"id": g.id(), "label": g.label(), "is_3d": g.is_3d()})).collect();
    Json(json!({
        "name": "GameFlow",
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "paths": st.paths,
        "play_base": format!("http://127.0.0.1:{}", *st.play_port.read().unwrap()),
        "tools": st.rt.tools(),
        "genres": genres,
        "themes": theme_ids(),
    }))
}

const MASK: &str = "••••";

async fn get_settings(State(st): State<S>) -> Json<Settings> {
    let mut s = st.settings.read().unwrap().clone();
    if !s.hf_token.is_empty() {
        s.hf_token = MASK.into();
    }
    for a in [&mut s.agents.text, &mut s.agents.code, &mut s.agents.image, &mut s.agents.sound] {
        if !a.api_key.is_empty() {
            a.api_key = MASK.into();
        }
    }
    Json(s)
}

async fn put_settings(State(st): State<S>, Json(mut new): Json<Settings>) -> R<Json<Value>> {
    {
        let old = st.settings.read().unwrap().clone();
        if new.hf_token == MASK {
            new.hf_token = old.hf_token.clone();
        }
        macro_rules! keep_key {
            ($f:ident) => {
                if new.agents.$f.api_key == MASK {
                    new.agents.$f.api_key = old.agents.$f.api_key.clone();
                }
            };
        }
        keep_key!(text);
        keep_key!(code);
        keep_key!(image);
        keep_key!(sound);
    }
    new.sanitize();
    new.save(&st.paths.settings_file)?;
    *st.settings.write().unwrap() = new;
    Ok(Json(json!({"ok": true})))
}

// ───────────────────────── projetos ─────────────────────────

async fn list_projects(State(st): State<S>) -> Json<Value> {
    Json(json!(st.projects.list()))
}

#[derive(Deserialize)]
struct CreateProject {
    name: String,
    genre: Option<String>,
}

async fn create_project(State(st): State<S>, Json(b): Json<CreateProject>) -> R<Json<Value>> {
    let genre = b.genre.as_deref().and_then(Genre::from_id);
    Ok(Json(json!(st.projects.create(&b.name, genre)?)))
}

async fn get_project(State(st): State<S>, Path(id): Path<String>) -> R<Json<Value>> {
    st.projects.get(&id).map(|m| Json(json!(m))).ok_or_else(|| not_found("projeto não encontrado"))
}

#[derive(Deserialize)]
struct PatchProject {
    name: String,
}

async fn patch_project(State(st): State<S>, Path(id): Path<String>, Json(b): Json<PatchProject>) -> R<Json<Value>> {
    Ok(Json(json!(st.projects.rename(&id, &b.name)?)))
}

async fn delete_project(State(st): State<S>, Path(id): Path<String>) -> R<Json<Value>> {
    if st.orch.active_for(&id).is_some() {
        return Err(ApiError(StatusCode::CONFLICT, "há uma geração em andamento neste projeto".into()));
    }
    st.projects.delete(&id)?;
    Ok(Json(json!({"ok": true})))
}

async fn tree(State(st): State<S>, Path(id): Path<String>) -> R<Json<Value>> {
    Ok(Json(json!(st.projects.tree(&id).map_err(|_| not_found("projeto não encontrado"))?)))
}

#[derive(Deserialize)]
struct PathQ {
    path: String,
}

async fn read_file(State(st): State<S>, Path(id): Path<String>, Query(q): Query<PathQ>) -> R<Response> {
    match st.projects.read_text(&id, &q.path) {
        Ok(content) => Ok(Json(json!({"path": q.path, "content": content})).into_response()),
        Err(e) => {
            let msg = format!("{e:#}");
            let status = if msg.contains("binário") { StatusCode::UNSUPPORTED_MEDIA_TYPE } else { StatusCode::BAD_REQUEST };
            Ok((status, Json(json!({"error": msg}))).into_response())
        }
    }
}

#[derive(Deserialize)]
struct WriteBody {
    path: String,
    content: String,
}

async fn write_file(State(st): State<S>, Path(id): Path<String>, Json(b): Json<WriteBody>) -> R<Json<Value>> {
    let p = b.path.trim_matches('/').to_string();
    if p.starts_with(HIDDEN_DIR) || p == META_FILE {
        return Err(ApiError(StatusCode::FORBIDDEN, "este arquivo é gerenciado pelo GameFlow".into()));
    }
    if p.ends_with(".json") {
        if let Err(e) = serde_json::from_str::<Value>(&b.content) {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, format!("JSON inválido: {e}")));
        }
        if p == "game.json" && serde_json::from_str::<crate::spec::GameSpec>(&b.content).is_err() {
            return Err(ApiError(StatusCode::UNPROCESSABLE_ENTITY, "game.json com formato inesperado (verifique tipos e nomes dos campos)".into()));
        }
    }
    st.projects.write(&id, &p, b.content.as_bytes())?;
    if p == "game.json" || p == "assets/manifest.json" {
        if let Some(dir) = st.projects.dir(&id) {
            let _ = templates::write_data_js(&dir);
        }
    }
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct EntryBody {
    path: String,
    kind: String,
}

async fn create_entry(State(st): State<S>, Path(id): Path<String>, Json(b): Json<EntryBody>) -> R<Json<Value>> {
    let p = b.path.trim_matches('/').to_string();
    if p.starts_with(HIDDEN_DIR) {
        return Err(ApiError(StatusCode::FORBIDDEN, "nome reservado".into()));
    }
    if st.projects.resolve(&id, &p)?.exists() {
        return Err(ApiError(StatusCode::CONFLICT, "já existe um item com esse nome".into()));
    }
    if b.kind == "dir" {
        st.projects.mkdir(&id, &p)?;
    } else {
        st.projects.write(&id, &p, b"")?;
    }
    Ok(Json(json!({"ok": true})))
}

async fn delete_entry(State(st): State<S>, Path(id): Path<String>, Query(q): Query<PathQ>) -> R<Json<Value>> {
    if q.path.trim_matches('/').starts_with(HIDDEN_DIR) {
        return Err(ApiError(StatusCode::FORBIDDEN, "item protegido".into()));
    }
    st.projects.remove(&id, &q.path)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct MoveBody {
    from: String,
    to: String,
}

async fn move_entry(State(st): State<S>, Path(id): Path<String>, Json(b): Json<MoveBody>) -> R<Json<Value>> {
    if b.from.trim_matches('/').starts_with(HIDDEN_DIR) || b.to.trim_matches('/').starts_with(HIDDEN_DIR) {
        return Err(ApiError(StatusCode::FORBIDDEN, "item protegido".into()));
    }
    st.projects.rename_path(&id, &b.from, &b.to)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct UploadQ {
    dir: Option<String>,
}

fn clean_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else if util::fold_accent(c).is_ascii_alphabetic() {
                util::fold_accent(c)
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_start_matches('.').to_string();
    if cleaned.is_empty() { "arquivo".into() } else { cleaned.chars().take(100).collect() }
}

async fn upload(State(st): State<S>, Path(id): Path<String>, Query(q): Query<UploadQ>, mut mp: Multipart) -> R<Json<Value>> {
    let dir = q.dir.unwrap_or_else(|| "references".into());
    let dir = if dir.trim_matches('/') == "." { String::new() } else { dir.trim_matches('/').to_string() };
    if dir.starts_with(HIDDEN_DIR) {
        return Err(ApiError(StatusCode::FORBIDDEN, "pasta reservada".into()));
    }
    let mut saved = vec![];
    while let Some(field) = mp.next_field().await.map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))? {
        let Some(name) = field.file_name().map(String::from) else { continue };
        let data = field.bytes().await.map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
        let mut fname = clean_filename(&name);
        let mut n = 2;
        let join = |d: &str, f: &str| if d.is_empty() { f.to_string() } else { format!("{d}/{f}") };
        while st.projects.resolve(&id, &join(&dir, &fname))?.exists() {
            let (stem, ext) = fname.rsplit_once('.').map(|(a, b)| (a.to_string(), format!(".{b}"))).unwrap_or((fname.clone(), String::new()));
            let stem = stem.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-').to_string();
            fname = format!("{stem}-{n}{ext}");
            n += 1;
        }
        let rel = join(&dir, &fname);
        st.projects.write(&id, &rel, &data)?;
        saved.push(rel);
    }
    Ok(Json(json!({"saved": saved})))
}

async fn export(State(st): State<S>, Path(id): Path<String>) -> R<Response> {
    let _ = templates::write_data_js(&st.projects.dir(&id).ok_or_else(|| not_found("projeto não encontrado"))?);
    let bytes = st.projects.export_zip(&id)?;
    Ok(Response::builder().header(header::CONTENT_TYPE, "application/zip").header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{id}.zip\"")).body(Body::from(bytes)).unwrap())
}

async fn chat(State(st): State<S>, Path(id): Path<String>) -> R<Json<Value>> {
    let dir = st.projects.dir(&id).ok_or_else(|| not_found("projeto não encontrado"))?;
    Ok(Json(json!(agents::chat_load(&dir))))
}

async fn open_folder(State(st): State<S>, Path(id): Path<String>) -> R<Json<Value>> {
    let dir = st.projects.dir(&id).ok_or_else(|| not_found("projeto não encontrado"))?;
    let opened = open::that_detached(&dir).is_ok();
    Ok(Json(json!({"ok": opened, "path": dir})))
}

// ───────────────────────── geração (agentes) ─────────────────────────

async fn generate(State(st): State<S>, Path(id): Path<String>, Json(req): Json<GenerateRequest>) -> R<Json<Value>> {
    let run = st.orch.start(&id, req).map_err(|e| {
        let m = format!("{e:#}");
        if m.contains("em andamento") { ApiError(StatusCode::CONFLICT, m) } else { ApiError(StatusCode::BAD_REQUEST, m) }
    })?;
    Ok(Json(json!({"run_id": run.id})))
}

async fn active_run(State(st): State<S>, Path(id): Path<String>) -> Json<Value> {
    Json(json!({"run_id": st.orch.active_for(&id).map(|r| r.id.clone())}))
}

#[derive(Deserialize)]
struct FromQ {
    from: Option<usize>,
}

async fn run_events(State(st): State<S>, Path(run_id): Path<String>, Query(q): Query<FromQ>) -> R<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let run = st.orch.get(&run_id).ok_or_else(|| not_found("execução não encontrada"))?;
    let rx = run.subscribe();
    let from = q.from.unwrap_or(0);
    let backlog = run.snapshot(from);
    let mut next = from + backlog.len();
    let live = BroadcastStream::new(rx).filter_map(|r| async move { r.ok() }).filter(move |e: &Envelope| {
        let fresh = e.seq >= next;
        if fresh {
            next = e.seq + 1;
        }
        futures_util::future::ready(fresh)
    });
    let stream = futures_util::stream::iter(backlog)
        .chain(live)
        .scan(false, |done, e| {
            if *done {
                return futures_util::future::ready(None);
            }
            if matches!(e.event, RunEvent::Done { .. }) {
                *done = true;
            }
            futures_util::future::ready(Some(e))
        })
        .map(|e| Ok(Event::default().json_data(&e).unwrap_or_else(|_| Event::default().data("{}"))));
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn run_cancel(State(st): State<S>, Path(run_id): Path<String>) -> R<Json<Value>> {
    let run = st.orch.get(&run_id).ok_or_else(|| not_found("execução não encontrada"))?;
    run.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(Json(json!({"ok": true})))
}

// ───────────────────────── Hugging Face / modelos ─────────────────────────

#[derive(Deserialize)]
struct SearchQ {
    q: String,
    task: Option<String>,
}

async fn hf_search(State(st): State<S>, Query(q): Query<SearchQ>) -> R<Json<Value>> {
    if q.q.trim().is_empty() {
        return Ok(Json(json!([])));
    }
    let r = st.hf.search(q.q.trim(), q.task.as_deref().unwrap_or("")).await.map_err(|e| ApiError(StatusCode::BAD_GATEWAY, format!("Não consegui buscar no Hugging Face: {e:#}")))?;
    Ok(Json(json!(r)))
}

#[derive(Deserialize)]
struct RepoQ {
    repo: String,
}

async fn hf_files(State(st): State<S>, Query(q): Query<RepoQ>) -> R<Json<Value>> {
    let r = st.hf.files(&q.repo).await.map_err(|e| ApiError(StatusCode::BAD_GATEWAY, format!("Não consegui listar os arquivos: {e:#}")))?;
    Ok(Json(json!(r)))
}

async fn hf_recommended() -> Json<Value> {
    Json(json!(hf::RECOMMENDED))
}

#[derive(Deserialize)]
struct DlBody {
    repo: String,
    file: String,
}

async fn hf_download(State(st): State<S>, Json(b): Json<DlBody>) -> R<Json<Value>> {
    Ok(Json(json!(st.hf.start_download(&b.repo, &b.file)?)))
}

async fn downloads(State(st): State<S>) -> Json<Value> {
    Json(json!(st.hf.downloads()))
}

async fn clear_downloads(State(st): State<S>) -> Json<Value> {
    st.hf.clear_finished();
    Json(json!({"ok": true}))
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

async fn cancel_download(State(st): State<S>, Json(b): Json<IdBody>) -> Json<Value> {
    Json(json!({"ok": st.hf.cancel(&b.id)}))
}

async fn local_models(State(st): State<S>) -> Json<Value> {
    Json(json!(st.hf.local_models()))
}

async fn delete_model(State(st): State<S>, Query(q): Query<PathQ>) -> R<Json<Value>> {
    st.hf.delete_local(&q.path)?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct TestBody {
    task: String,
}

async fn agent_test(State(st): State<S>, Json(b): Json<TestBody>) -> R<Json<Value>> {
    let task = Task::parse(&b.task).ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "tarefa desconhecida".into()))?;
    Ok(Json(match st.rt.test(task).await {
        Ok(m) => json!({"ok": true, "message": m}),
        Err(e) => json!({"ok": false, "message": format!("{e:#}")}),
    }))
}

// ───────────────────────── servidor de jogos (origem isolada) ─────────────────────────

#[derive(Clone)]
struct PlayState {
    projects: Projects,
}

pub fn play_router(projects: Projects) -> Router {
    Router::new().fallback(play_file).layer(middleware::from_fn(guard)).with_state(PlayState { projects })
}

async fn play_file(State(ps): State<PlayState>, req: Request) -> Response {
    let raw = req.uri().path().trim_start_matches('/').to_string();
    let decoded = percent_decode(&raw);
    let mut parts = decoded.splitn(2, '/');
    let id = parts.next().unwrap_or("");
    let mut rel = parts.next().unwrap_or("").to_string();
    if rel.is_empty() || rel.ends_with('/') {
        rel.push_str("index.html");
    }
    if rel.starts_with(HIDDEN_DIR) || ps.projects.dir(id).is_none() {
        return (StatusCode::NOT_FOUND, "não encontrado").into_response();
    }
    let Ok(path) = ps.projects.resolve(id, &rel) else { return (StatusCode::BAD_REQUEST, "caminho inválido").into_response() };
    let path = if path.is_dir() { path.join("index.html") } else { path };
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            let mut resp = Response::builder().header(header::CONTENT_TYPE, mime.as_ref()).body(Body::from(bytes)).unwrap();
            let h = resp.headers_mut();
            // Jogos gerados por IA não precisam de rede: bloqueia conexões externas por padrão.
            h.insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("default-src 'self' data: blob:; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data: blob:; media-src 'self' data: blob:; font-src 'self' data:"),
            );
            resp
        }
        Err(_) => (StatusCode::NOT_FOUND, "arquivo não encontrado").into_response(),
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

// ───────────────────────── execução ─────────────────────────

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}

/// Sobe a interface e o servidor de jogos. Bloqueia até Ctrl+C.
pub async fn serve(st: S, host: &str, port: u16, open_browser: bool) -> anyhow::Result<()> {
    let mut listener = None;
    let mut chosen = port;
    for p in port..port.saturating_add(20) {
        if let Ok(l) = TcpListener::bind(format!("{host}:{p}")).await {
            listener = Some(l);
            chosen = p;
            break;
        }
    }
    let listener = listener.ok_or_else(|| anyhow::anyhow!("não consegui abrir nenhuma porta entre {port} e {}", port + 19))?;
    let play_listener = TcpListener::bind(format!("{host}:0")).await?;
    *st.port.write().unwrap() = chosen;
    *st.play_port.write().unwrap() = play_listener.local_addr()?.port();

    let url = format!("http://{host}:{chosen}");
    println!("\n  GameFlow {} rodando em {url}", env!("CARGO_PKG_VERSION"));
    println!("  Projetos: {}", st.paths.projects.display());
    println!("  (Ctrl+C para encerrar)\n");
    if open_browser {
        let _ = open::that_detached(&url);
    }

    let app = router(st.clone());
    let play = play_router(st.projects.clone());
    let (tx, _) = tokio::sync::broadcast::channel::<()>(1);
    let (mut rx1, mut rx2) = (tx.subscribe(), tx.subscribe());
    let main_srv = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(async move {
        let _ = rx1.recv().await;
    });
    let play_srv = axum::serve(play_listener, play.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(async move {
        let _ = rx2.recv().await;
    });
    let sig = async {
        shutdown_signal().await;
        let _ = tx.send(());
    };
    let (a, b, _) = tokio::join!(main_srv, play_srv, sig);
    a?;
    b?;
    st.rt.shutdown().await;
    Ok(())
}
