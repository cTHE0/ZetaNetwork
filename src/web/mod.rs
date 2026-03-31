use axum::{
    Router,
    routing::{get, post, delete},
    extract::{State, Path},
    response::{Html, IntoResponse, Json},
    http::StatusCode,
};
use tower_http::cors::{CorsLayer, Any};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::post::Post;
use crate::network::NetworkState;

pub struct AppState {
    pub network: Arc<NetworkState>,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Json<ApiResponse<T>> {
        Json(ApiResponse {
            success: true,
            data: Some(data),
            error: None,
        })
    }

    fn err(msg: &str) -> Json<ApiResponse<T>> {
        Json(ApiResponse {
            success: false,
            data: None,
            error: Some(msg.to_string()),
        })
    }
}

#[derive(Deserialize)]
pub struct CreatePostRequest {
    content: String,
}

#[derive(Deserialize)]
pub struct AddSubscriptionRequest {
    pubkey: String,
}

#[derive(Serialize)]
pub struct IdentityResponse {
    pubkey: String,
}

#[derive(Serialize)]
pub struct PostResponse {
    id: String,
    author_pubkey: String,
    content: String,
    timestamp: u64,
    verified: bool,
}

impl From<Post> for PostResponse {
    fn from(p: Post) -> Self {
        let verified = p.verify();
        PostResponse {
            id: p.id,
            author_pubkey: p.author_pubkey,
            content: p.content,
            timestamp: p.timestamp,
            verified,
        }
    }
}

#[derive(Serialize)]
pub struct PeerResponse {
    addr: String,
    pubkey: Option<String>,
    last_seen: u64,
}

pub async fn start_web_server(network: Arc<NetworkState>, port: u16) {
    let state = Arc::new(AppState { network });

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/identity", get(get_identity))
        .route("/api/posts", get(get_posts))
        .route("/api/posts", post(create_post))
        .route("/api/subscriptions", get(get_subscriptions))
        .route("/api/subscriptions", post(add_subscription))
        .route("/api/subscriptions/{pubkey}", delete(remove_subscription))
        .route("/api/peers", get(get_peers))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    println!("\n[WEB] Interface disponible sur http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index_handler() -> impl IntoResponse {
    Html(include_str!("static/index.html"))
}

async fn get_identity(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pubkey = state.network.keypair.public_hex();
    ApiResponse::ok(IdentityResponse { pubkey })
}

async fn get_posts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let storage = state.network.storage.lock().await;

    // Récupérer les abonnements
    let subs = storage.get_subscriptions().unwrap_or_default();

    // Récupérer les posts des abonnements + mes propres posts
    let my_pubkey = state.network.keypair.public_hex();
    let mut all_authors = subs;
    all_authors.push(my_pubkey);

    let posts = storage.get_posts_by_authors(&all_authors, 100).unwrap_or_default();
    let responses: Vec<PostResponse> = posts.into_iter().map(PostResponse::from).collect();

    ApiResponse::ok(responses)
}

async fn create_post(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreatePostRequest>,
) -> impl IntoResponse {
    if req.content.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, ApiResponse::<PostResponse>::err("Content cannot be empty"));
    }
    if req.content.len() > 500 {
        return (StatusCode::BAD_REQUEST, ApiResponse::<PostResponse>::err("Content too long (max 500 chars)"));
    }

    // Créer le post
    let post = Post::new(req.content, &state.network.keypair);

    // Sauvegarder localement
    {
        let storage = state.network.storage.lock().await;
        if let Err(e) = storage.save_post(&post) {
            return (StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<PostResponse>::err(&format!("Failed to save: {}", e)));
        }
    }

    // Diffuser aux peers
    state.network.broadcast_post(&post).await;

    (StatusCode::OK, ApiResponse::ok(PostResponse::from(post)))
}

async fn get_subscriptions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let storage = state.network.storage.lock().await;
    let subs = storage.get_subscriptions().unwrap_or_default();
    ApiResponse::ok(subs)
}

async fn add_subscription(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddSubscriptionRequest>,
) -> impl IntoResponse {
    if req.pubkey.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, ApiResponse::<String>::err("Pubkey cannot be empty"));
    }

    let storage = state.network.storage.lock().await;
    if let Err(e) = storage.add_subscription(&req.pubkey) {
        return (StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<String>::err(&format!("Failed: {}", e)));
    }
    (StatusCode::OK, ApiResponse::ok(req.pubkey))
}

async fn remove_subscription(
    State(state): State<Arc<AppState>>,
    Path(pubkey): Path<String>,
) -> impl IntoResponse {
    let storage = state.network.storage.lock().await;
    if let Err(e) = storage.remove_subscription(&pubkey) {
        return (StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::err(&format!("Failed: {}", e)));
    }
    (StatusCode::OK, ApiResponse::ok(()))
}

async fn get_peers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let storage = state.network.storage.lock().await;
    let peers = storage.get_peers().unwrap_or_default();
    let responses: Vec<PeerResponse> = peers
        .into_iter()
        .map(|(addr, pubkey, last_seen)| PeerResponse { addr, pubkey, last_seen })
        .collect();
    ApiResponse::ok(responses)
}
