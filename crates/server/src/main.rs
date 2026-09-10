//! The sync server.
//!
//! There is almost nothing here, and that is the point: `petros::Server` owns
//! no socket, no runtime and no thread, so putting it behind HTTP is a handler
//! you mount rather than a server you start. Everything below except the
//! `/sync` line is an ordinary axum program — which is what it should look
//! like when you want your own routes, your own middleware and your own
//! authentication around it.
//!
//! It also serves the browser client, when there is one to serve. One port for
//! both, because they are one deployment: the socket at `/sync` and the client
//! that talks to it at `/`, so a phone or another laptop needs one address and
//! no CORS. `HARKEN_WEB` points at the directory; without it the server is
//! exactly what it was, and says so.
//!
//! ```text
//! just serve                     # 127.0.0.1:8787, a file in the temp dir
//! just serve 0.0.0.0:8787        # reachable from a phone on the same network
//! HARKEN_WEB=clients/iced/web just serve   # …with the browser client on /
//! ```

use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::Router;
use harken::HarkenApp;
use petros_axum::Hub;
use tower_http::services::{ServeDir, ServeFile};

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
    let mut app = Router::new()
        .route("/sync", get(petros_axum::sync::<HarkenApp>))
        .route("/healthz", get(healthz));

    // The routes above are matched first, so `/sync` stays the socket however
    // the directory is laid out. Anything else falls through to the files, and
    // anything the files do not have falls back to `index.html` — which is what
    // makes a reload of a deep link work in a single-page client.
    let web = std::env::var_os("HARKEN_WEB");
    if let Some(dir) = &web {
        let index = std::path::Path::new(dir).join("index.html");
        app = app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    }
    let app = app.with_state(hub);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("harken-server on ws://{addr}/sync — {}", path.display());
    match &web {
        Some(dir) => println!(
            "  browser client on http://{addr}/ — {}",
            dir.to_string_lossy()
        ),
        // Said out loud, because a server that silently serves nothing on `/`
        // looks broken in exactly the same way as one that is misconfigured.
        None => println!("  no browser client: set HARKEN_WEB to a built one"),
    }
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
