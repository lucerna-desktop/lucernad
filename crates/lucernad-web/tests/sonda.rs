//! A sonda do D-Bus de verdade (o `zbus` falando com um `dbus-daemon`), com
//! um NetworkManager falso registrado por este processo. Rode pelo
//! `tests/isolado.sh`, que sobe um D-Bus do sistema de mentira num container:
//! fora dele, a trava `isolated()` recusa, e o NetworkManager da máquina
//! nunca é tocado.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lucernad_web::net::{Connectivity, Link, NmConnectivity};
use lucernad_web::testing::isolated;
use tokio::sync::watch::Receiver;
use zbus::connection::Builder;

const NM: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";

/// A janela de cada troca.
const WAIT: Duration = Duration::from_secs(2);

struct FakeNm {
    connectivity: Arc<Mutex<u32>>,
    /// Quantas vezes o `Connectivity` foi lido.
    reads: Arc<AtomicUsize>,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl FakeNm {
    #[zbus(property)]
    fn connectivity(&self) -> u32 {
        self.reads.fetch_add(1, Ordering::SeqCst);
        *self.connectivity.lock().unwrap()
    }
}

/// Outra interface no mesmo caminho do NetworkManager.
struct OtherIface;

#[zbus::interface(name = "org.freedesktop.NetworkManager.Outra")]
impl OtherIface {
    #[zbus(property)]
    fn coisa(&self) -> u32 {
        7
    }
}

/// Espera (até [`WAIT`]) o `watch` chegar a `want`.
async fn until(rx: &mut Receiver<Link>, want: Link) {
    let reached = matches!(
        tokio::time::timeout(WAIT, rx.wait_for(|link| *link == want)).await,
        Ok(Ok(_))
    );
    assert!(reached, "esperava {want:?}, ficou {:?}", *rx.borrow());
}

/// Registra o NetworkManager falso com `connectivity`.
async fn serve(connectivity: Arc<Mutex<u32>>) -> zbus::Connection {
    serve_counting(connectivity, Arc::default()).await
}

/// O NetworkManager falso contando as leituras em `reads`, com outra
/// interface no mesmo caminho.
async fn serve_counting(
    connectivity: Arc<Mutex<u32>>,
    reads: Arc<AtomicUsize>,
) -> zbus::Connection {
    Builder::system()
        .unwrap()
        .name(NM)
        .unwrap()
        .serve_at(
            NM_PATH,
            FakeNm {
                connectivity,
                reads,
            },
        )
        .unwrap()
        .serve_at(NM_PATH, OtherIface)
        .unwrap()
        .build()
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "só no D-Bus isolado (tests/isolado.sh)"]
async fn starts_without_network_manager() {
    isolated();
    // Sem o nome na partida: desconhecida, e o NameOwnerChanged pega a chegada.
    let nm = NmConnectivity::system();
    let mut rx = nm.watch();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(*rx.borrow_and_update(), Link::Unknown);
    let service = serve(Arc::new(Mutex::new(1))).await;
    until(&mut rx, Link::Offline).await;
    service.release_name(NM).await.unwrap();
    until(&mut rx, Link::Unknown).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "só no D-Bus isolado (tests/isolado.sh)"]
async fn follows_network_manager() {
    isolated();
    let connectivity = Arc::new(Mutex::new(4));
    let service = serve(connectivity.clone()).await;

    let nm = NmConnectivity::system();
    let mut rx = nm.watch();
    until(&mut rx, Link::Online).await;
    // A segunda chamada divide o mesmo acompanhamento.
    assert_eq!(*nm.watch().borrow(), Link::Online);

    // A conectividade cai para "nenhuma": o PropertiesChanged avisa.
    *connectivity.lock().unwrap() = 1;
    let iface = service
        .object_server()
        .interface::<_, FakeNm>(NM_PATH)
        .await
        .unwrap();
    iface
        .get()
        .await
        .connectivity_changed(iface.signal_emitter())
        .await
        .unwrap();
    until(&mut rx, Link::Offline).await;

    // O NetworkManager some: sem o nome, desconhecida.
    service.release_name(NM).await.unwrap();
    until(&mut rx, Link::Unknown).await;

    // Volta com a conectividade completa: o NameOwnerChanged relê.
    *connectivity.lock().unwrap() = 4;
    service.request_name(NM).await.unwrap();
    until(&mut rx, Link::Online).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "só no D-Bus isolado (tests/isolado.sh)"]
async fn other_interfaces_do_not_reread() {
    isolated();
    let reads = Arc::new(AtomicUsize::new(0));
    let connectivity = Arc::new(Mutex::new(4));
    let service = serve_counting(connectivity.clone(), reads.clone()).await;
    let nm = NmConnectivity::system();
    let mut rx = nm.watch();
    until(&mut rx, Link::Online).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let before = reads.load(Ordering::SeqCst);

    // O PropertiesChanged de outra interface no mesmo caminho: nada é relido.
    let other = service
        .object_server()
        .interface::<_, OtherIface>(NM_PATH)
        .await
        .unwrap();
    other
        .get()
        .await
        .coisa_changed(other.signal_emitter())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        reads.load(Ordering::SeqCst),
        before,
        "releu por outra interface"
    );

    // O do NetworkManager relê.
    *connectivity.lock().unwrap() = 1;
    let iface = service
        .object_server()
        .interface::<_, FakeNm>(NM_PATH)
        .await
        .unwrap();
    iface
        .get()
        .await
        .connectivity_changed(iface.signal_emitter())
        .await
        .unwrap();
    until(&mut rx, Link::Offline).await;
}
