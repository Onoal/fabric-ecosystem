use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::thread::JoinHandle;

use fabric_core::Health;
use fabric_resource_ingress::{IngressRoute, IngressRouteId, LocalHttpIngressAccess};
use fabric_resource_service::{ServiceContract, ServiceId};

use crate::adapter::shutdown::ShutdownController;

pub(crate) struct SharedPingoraState {
    pub(crate) inner: Mutex<PingoraState>,
}

pub(crate) struct PingoraState {
    pub(crate) health: Health,
    pub(crate) started: bool,
    pub(crate) service: Option<ServiceContract>,
    pub(crate) routes: BTreeMap<IngressRouteId, IngressRoute>,
    pub(crate) routes_by_service: BTreeMap<ServiceId, IngressRouteId>,
    pub(crate) local_http_access: BTreeMap<IngressRouteId, LocalHttpIngressAccess>,
    pub(crate) runtime: Option<PingoraRuntime>,
    pub(crate) previous_port: Option<u16>,
}

pub(crate) struct PingoraRuntime {
    pub(crate) listen_address: SocketAddr,
    pub(crate) shutdown: ShutdownController,
    pub(crate) thread: JoinHandle<()>,
}

impl SharedPingoraState {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(PingoraState {
                health: Health::Unavailable,
                started: false,
                service: None,
                routes: BTreeMap::new(),
                routes_by_service: BTreeMap::new(),
                local_http_access: BTreeMap::new(),
                runtime: None,
                previous_port: None,
            }),
        }
    }
}
