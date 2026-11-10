//! O `SessionBus` numa conexão ponto a ponto de mentira, sem o D-Bus da
//! sessão: o outro lado aceita a conexão e nunca responde.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lucernad_spectrum::dbus::{Bus, Identity, SessionBus};
use tokio::net::UnixStream;
use zbus::connection::Builder;
use zbus::{Connection, Guid};

const SPOTIFY: &str = "org.mpris.MediaPlayer2.spotify";

/// Uma conexão com um par que nunca responde; o par fica em `peers`.
async fn mute_peer(peers: &Mutex<Vec<Connection>>) -> zbus::Result<Connection> {
    let (ours, theirs) = UnixStream::pair().map_err(|e| zbus::Error::InputOutput(e.into()))?;
    let server = async {
        Builder::unix_stream(theirs)
            .server(Guid::generate())?
            .p2p()
            .build()
            .await
    };
    let client = Builder::unix_stream(ours).p2p().build();
    let (server, client) = tokio::join!(server, client);
    peers.lock().unwrap().push(server?);
    client
}

#[tokio::test]
async fn a_bus_that_stops_answering_is_reconnected() {
    let connects = Arc::new(AtomicUsize::new(0));
    let peers = Arc::new(Mutex::new(Vec::new()));
    let (count, kept) = (connects.clone(), peers.clone());
    let bus = SessionBus::with_connect(
        Duration::from_millis(100),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            let kept = kept.clone();
            Box::pin(async move { mute_peer(&kept).await })
        }),
    );
    let only_name = Identity {
        name: SPOTIFY.into(),
        ..Identity::default()
    };
    assert_eq!(bus.identify(SPOTIFY).await, only_name);
    assert_eq!(connects.load(Ordering::SeqCst), 1);
    // O prazo do barramento estourou: a próxima identificação reconecta.
    assert_eq!(bus.identify(SPOTIFY).await, only_name);
    assert_eq!(connects.load(Ordering::SeqCst), 2);
}
