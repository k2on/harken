//! The sync server.
//!
//! There is almost nothing here, and that is the point: `petros::Server` owns
//! no socket, no runtime and no thread, so putting it behind HTTP is a handler
//! you mount rather than a server you start. Everything below except the
//! `/sync` line is an ordinary axum program — which is what it should look
//! like when you want your own routes, your own middleware and your own
//! authentication around it.
//!
//! ```text
//! just serve                     # 127.0.0.1:8787, a file in the temp dir
//! just serve 0.0.0.0:8787        # reachable from a phone on the same network
//! ```

use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::Router;
use harken::HarkenApp;
use petros_axum::Hub;

type Shared = Arc<Hub<HarkenApp>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8787".to_string());
    // The same path the demo peers use, so `just serve` and `just iced` meet
    // without being told where.
    let path = std::env::temp_dir().join("harken-server.db");

    let hub = Hub::<HarkenApp>::open(petros::open_path(&path)?)?;
    let app = Router::new()
        .route("/sync", get(petros_axum::sync::<HarkenApp>))
        .route("/healthz", get(healthz))
        .with_state(hub);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("harken-server on ws://{addr}/sync — {}", path.display());
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            println!();
        })
        .await?;
    Ok(())
}

/// An ordinary route beside the socket, reading the same state.
///
/// `Hub::server()` is the engine itself; holding it blocks every connection, so
/// this takes what it needs and lets go.
async fn healthz(State(hub): State<Shared>) -> String {
    let head = hub.server().head();
    format!("ok\nlog: {head}\npeers: {}\n", hub.connected())
}
