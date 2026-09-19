//! The sync server.
//!
//! There is almost nothing here, and that is the point: `petros::Server` owns
//! no socket, no runtime and no thread, so putting it behind HTTP is a handler
//! you mount rather than a server you start. Everything below except the
//! `/sync` line and the `/auth/*` routes is an ordinary axum program.
//!
//! It signs people in, because the engine holds every entry to who pushed it
//! and somebody has to say who that is. The server is the only OpenID Connect
//! client: it has the secret, it talks to the provider, and it hands each peer
//! a session token of its own — so the desktop, the browser and the phone all
//! sign in the same way, by opening `/auth/login` and exchanging one code. See
//! `petros_auth` for the flow. Without a provider it refuses to start, unless
//! told it is on a laptop, where `HARKEN_DEV_AUTH=1` signs anyone in as a name.
//!
//! It also serves the browser client, when there is one to serve. One port for
//! both, because they are one deployment: the socket at `/sync`, the login
//! beside it, and the client that talks to both at `/`, so a phone or another
//! laptop needs one address and no CORS. `HARKEN_WEB` points at the directory;
//! without it the server is exactly what it was, and says so.
//!
//! ```text
//! HARKEN_DEV_AUTH=1 harken-server              # 127.0.0.1:8787, a file in the temp dir
//! HARKEN_DEV_AUTH=1 harken-server 0.0.0.0:8787 # reachable from a phone on the same network
//! HARKEN_OIDC_ISSUER=https://auth.example.com HARKEN_OIDC_CLIENT_ID=harken \
//!   HARKEN_OIDC_CLIENT_SECRET_FILE=/run/credentials/harken/oidc-secret \
//!   HARKEN_PUBLIC_URL=https://harken.example.com HARKEN_WEB=/path/to/harken-web \
//!   harken-server 127.0.0.1:8787               # what the NixOS module runs
//! ```

use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::Router;
use harken::HarkenApp;
use harken_server::assistant::ha;
use harken_server::{library, listening};
use petros_auth::oidc::Provider;
use petros_auth::server::{Auth, Mode};
use petros_auth::session::SessionStore;
use petros_axum::Hub;
use tower_http::services::{ServeDir, ServeFile};

type Shared = Arc<Hub<HarkenApp>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8787".to_string());
    // The same path the demo peers use, so `nix run .#serve` and `nix run .#iced` meet
    // without being told where. Sessions beside it, in their own file: they
    // are not part of the log and never leave this machine.
    let dir = std::env::temp_dir();
    let path = dir.join("harken-server.db");
    let mut sessions = SessionStore::open(petros::open_path(dir.join("harken-sessions.db"))?)?;

    // The scanner signs in like everything else. The engine holds every entry
    // to a session, and "the server wrote it" is not an exemption — so the
    // server mints one for itself, here, while it still has the store.
    let library_login = sessions.issue(&petros_auth::Account {
        id: library::ACCOUNT.to_string(),
        name: "Library".into(),
        email: String::new(),
    })?;

    // Where a browser reaches this server, which is where the provider sends
    // people back to. Behind a reverse proxy it is the proxy's address.
    let public_url = env("HARKEN_PUBLIC_URL").unwrap_or_else(|| format!("http://{addr}"));
    let mode = mode().await?;
    let mut auth = Auth::new(sessions, mode, &public_url)
        // The phone's URL scheme, from `mobile/app.config.ts`.
        .allow_redirect("harken://");
    for prefix in env("HARKEN_REDIRECTS").unwrap_or_default().split(',') {
        if !prefix.trim().is_empty() {
            auth = auth.allow_redirect(prefix.trim());
        }
    }
    let auth = Arc::new(auth);

    // The house, if there is one — read before the hub, because the bridge
    // has to be listening for rooms before any of them can open.
    let house = assistant()?;

    // What each account is listening to, and where. It is the hub's realtime
    // machine now rather than a socket of its own: `petros::live` carries it
    // on the same wire as the log, in a room per account, held in memory. The
    // log is permanent and an afternoon of pauses and skips is not worth
    // replaying tomorrow. See `harken::listening`.
    let mut desk = listening::Desk::new();
    // Registered *before* the desk is handed over, because after that it
    // belongs to the engine. The rooms already open come back at once, so a
    // bridge is never told about a room it missed.
    let rooms = house.as_ref().map(|_| {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        desk.watch(tx);
        rx
    });

    let hub = Hub::<HarkenApp>::open_live(petros::open_path(&path)?, auth.authenticator(), desk)?;

    // The media directory, if there is one. Two halves that have to agree on
    // one string: the scanner writes each track's path *relative to this
    // directory* into `file`, and `/media/` serves that same path back — so a
    // client plays what the scanner wrote without either of them knowing where
    // the directory is. `ServeDir` answers range requests, which is what makes
    // seeking work rather than re-downloading.
    //
    // One root for every kind rather than one per kind, because `file` is on
    // the kind-neutral side of the schema and its base has to be kind-neutral
    // too. Music is `music/` under it and an episode will be a sibling, so a
    // song's path reads `music/…` and its URL `/media/music/…`.
    let media = env("HARKEN_MEDIA").map(std::path::PathBuf::from);

    let mut app = Router::new()
        .route("/sync", get(petros_axum::sync::<HarkenApp>))
        .route("/healthz", get(healthz))
        .with_state(hub.clone())
        .merge(petros_auth::server::router(auth.clone()));
    if let Some(dir) = &media {
        app = app.nest_service("/media", ServeDir::new(dir));
    }

    // Every `media_player` Home Assistant knows about becomes a device in
    // every listening session — a Sonos, a Chromecast, a television — because
    // a speaker is a device and nothing in the protocol says the far end has
    // to be somebody's screen. It stands in each room as a peer of the
    // realtime channel, which is how it is a device rather than a client.
    //
    // Kept alive for the life of the process: dropping it stops the bridge.
    let _assistant = match (house, rooms) {
        (Some(config), Some(rooms)) => {
            println!("  {} player(s) from {}", config.players.len(), config.url);
            Some(ha::start(config, hub.clone(), rooms))
        }
        _ => None,
    };

    // Kept alive for the life of the process: dropping it stops the watch.
    let _scanner = match &media {
        Some(dir) => Some(library::Scanner::start(
            dir.clone(),
            hub.clone(),
            path.with_extension("library.db"),
            library_login,
        )?),
        None => None,
    };

    // The routes above are matched first, so `/sync` stays the socket however
    // the directory is laid out. Anything else falls through to the files, and
    // anything the files do not have falls back to `index.html` — which is what
    // makes a reload of a deep link work in a single-page client.
    let web = std::env::var_os("HARKEN_WEB");
    if let Some(dir) = &web {
        let index = std::path::Path::new(dir).join("index.html");
        app = app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("harken-server on ws://{addr}/sync — {}", path.display());
    println!("  one listening session per account, on that same socket");
    match auth.mode() {
        Mode::Oidc(provider) => println!("  signing in through {}", provider.issuer()),
        // Loudly, every time: a server that takes people's word for who they
        // are is fine on a laptop and nowhere else.
        Mode::Dev => println!("  DEV AUTH: anyone is whoever they say they are"),
    }
    match &media {
        Some(dir) => println!("  media from {} (music in music/)", dir.display()),
        // Said out loud for the same reason as the client below: a server with
        // no media looks exactly like one whose directory is misconfigured.
        None => println!("  no media: set HARKEN_MEDIA to a directory"),
    }
    match &web {
        Some(dir) => println!(
            "  browser client on {public_url}/ — {}",
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

/// The house, from the environment, or nothing.
///
/// All four together or none of them, the way the OpenID Connect three are:
/// half a configuration is a server that starts and quietly does not do the
/// thing it was configured for.
///
/// The token is a *file* for the same reason the client secret is — systemd
/// hands it over as a credential, so it is never in a process listing, a unit
/// file or a store path.
fn assistant() -> Result<Option<ha::Config>, Box<dyn std::error::Error>> {
    let url = env("HARKEN_HA_URL");
    let token_file = env("HARKEN_HA_TOKEN_FILE");
    let players = env("HARKEN_HA_PLAYERS");
    match (url, token_file, players) {
        (Some(url), Some(file), Some(players)) => {
            let token = std::fs::read_to_string(&file)
                .map_err(|e| format!("cannot read HARKEN_HA_TOKEN_FILE {file}: {e}"))?
                .trim()
                .to_string();
            // `media_player.kitchen=Kitchen,media_player.study` — a name after
            // an `=` when the entity id is not what you would call it out
            // loud, and the id itself when it is.
            let players = players
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| match p.split_once('=') {
                    Some((id, name)) => (id.trim().to_string(), name.trim().to_string()),
                    None => (p.to_string(), pretty(p)),
                })
                .collect::<Vec<_>>();
            if players.is_empty() {
                return Err("HARKEN_HA_PLAYERS names no players".into());
            }
            // Where a *speaker* fetches from, which is not necessarily where a
            // phone does: the phone may be on a public address while the
            // speaker only knows one on the LAN.
            let media = env("HARKEN_HA_MEDIA")
                .or_else(|| env("HARKEN_PUBLIC_URL"))
                .ok_or("HARKEN_HA_MEDIA: speakers need an address they can fetch bytes from")?;
            Ok(Some(ha::Config {
                url,
                token,
                players,
                media,
            }))
        }
        (None, None, None) => Ok(None),
        _ => Err(
            "HARKEN_HA_URL, HARKEN_HA_TOKEN_FILE and HARKEN_HA_PLAYERS go together; \
                  set all three"
                .into(),
        ),
    }
}

/// `media_player.the_kitchen` as `The kitchen`. A guess, and overridden by
/// writing the name out — but a picker full of entity ids is a picker for
/// somebody who already knows what is in their house.
fn pretty(entity: &str) -> String {
    let tail = entity
        .rsplit('.')
        .next()
        .unwrap_or(entity)
        .replace('_', " ");
    let mut chars = tail.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => entity.to_string(),
    }
}

/// How people sign in, from the environment: a provider, or a laptop.
async fn mode() -> Result<Mode, Box<dyn std::error::Error>> {
    let issuer = env("HARKEN_OIDC_ISSUER");
    let client_id = env("HARKEN_OIDC_CLIENT_ID");
    let secret_file = env("HARKEN_OIDC_CLIENT_SECRET_FILE");
    match (issuer, client_id, secret_file) {
        (Some(issuer), Some(client_id), Some(file)) => {
            // A file rather than a variable, so the secret is never in a
            // process listing or a unit file: systemd hands it over as a
            // credential and only this process can read it.
            let secret = std::fs::read_to_string(&file)
                .map_err(|e| format!("cannot read HARKEN_OIDC_CLIENT_SECRET_FILE {file}: {e}"))?
                .trim()
                .to_string();
            let scopes = env("HARKEN_OIDC_SCOPES").unwrap_or_else(|| "openid profile email".into());
            // Discovery is one blocking request to the provider, once.
            let provider = tokio::task::spawn_blocking(move || {
                let scopes: Vec<&str> = scopes.split_whitespace().collect();
                Provider::discover(&issuer, &client_id, &secret, &scopes)
            })
            .await??;
            Ok(Mode::Oidc(provider))
        }
        (None, None, None) if env("HARKEN_DEV_AUTH").is_some() => Ok(Mode::Dev),
        (None, None, None) => Err("nobody can sign in: set HARKEN_OIDC_ISSUER, \
             HARKEN_OIDC_CLIENT_ID and HARKEN_OIDC_CLIENT_SECRET_FILE, or HARKEN_DEV_AUTH=1 \
             on a laptop"
            .into()),
        _ => Err(
            "HARKEN_OIDC_ISSUER, HARKEN_OIDC_CLIENT_ID and HARKEN_OIDC_CLIENT_SECRET_FILE \
                  go together; set all three"
                .into(),
        ),
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// An ordinary route beside the socket, reading the same state.
///
/// `Hub::server()` is the engine itself; holding it blocks every connection, so
/// this takes what it needs and lets go.
async fn healthz(State(hub): State<Shared>) -> String {
    let head = hub.server().head();
    format!("ok\nlog: {head}\npeers: {}\n", hub.connected())
}
