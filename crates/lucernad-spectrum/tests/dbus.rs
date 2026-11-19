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

/// Um `SessionBus` com o prazo `timeout` sobre pares mudos, e a conta das
/// conexões abertas.
fn mute_bus(timeout: Duration) -> (SessionBus, Arc<AtomicUsize>) {
    let connects = Arc::new(AtomicUsize::new(0));
    let peers = Arc::new(Mutex::new(Vec::new()));
    let (count, kept) = (connects.clone(), peers.clone());
    let bus = SessionBus::with_connect(
        timeout,
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            let kept = kept.clone();
            Box::pin(async move { mute_peer(&kept).await })
        }),
    );
    (bus, connects)
}

#[tokio::test]
async fn a_bus_that_stops_answering_is_reconnected() {
    let (bus, connects) = mute_bus(Duration::from_millis(100));
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

#[tokio::test]
async fn the_outer_deadline_still_reconnects() {
    // Como no módulo: o mesmo prazo por fora do `identify`, que começa antes
    // do de dentro e vence primeiro.
    let deadline = Duration::from_millis(100);
    let (bus, connects) = mute_bus(deadline);
    let outer = tokio::time::timeout(deadline, bus.identify(SPOTIFY)).await;
    assert!(outer.is_err(), "o prazo de fora venceu primeiro");
    assert_eq!(connects.load(Ordering::SeqCst), 1);
    let _ = tokio::time::timeout(deadline, bus.identify(SPOTIFY)).await;
    assert_eq!(connects.load(Ordering::SeqCst), 2);
}
