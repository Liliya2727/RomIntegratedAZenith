// ---------------- hosted service: vendor.azenith.IAZenithService ----------------
// rsbinder talks to the kernel binder driver directly, so this needs no
// libbinder_ndk and no hand-rolled parcel code. Transaction codes come from the
// generated bindings; the .aidl under Cargo/aidl is the single source of truth
// and must be appended to in order (declaration order == transaction order).

use rsbinder::{hub, BinderResult, Interface};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use vendor::azenith::IAZenithService::{BnAZenithService, IAZenithService};

use crate::common::log::{comp, error, info};
use crate::common::paths::STATE_FILE;
use crate::daemon::applist;

pub(crate) const IFACE_VERSION: i32 = 1;

const SERVICE_NAME: &str = "vendor.azenith.IAZenithService/default";

struct AZenithService;

impl Interface for AZenithService {}

impl IAZenithService for AZenithService {
    fn getApiVersion(&self) -> BinderResult<i32> {
        Ok(IFACE_VERSION)
    }

    fn setDisabled(&self, disabled: bool) -> BinderResult<bool> {
        let v = if disabled { "1" } else { "0" };
        crate::common::prop::setprop("persist.sys.azenithIntg.disabletweak", v);
        Ok(true)
    }

    fn isDisabled(&self) -> BinderResult<bool> {
        Ok(crate::common::prop::getprop("persist.sys.azenithIntg.disabletweak") == "1")
    }

    fn setConfig(&self, key: &str, value: &str) -> BinderResult<bool> {
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            error!(comp::DAEMON, "aidl: rejected config key '{key}'");
            return Ok(false);
        }
        crate::common::prop::setprop(&format!("persist.sys.azenithConfIntg.{key}"), value);
        Ok(true)
    }

    fn addGame(&self, pkg: &str) -> BinderResult<bool> {
        Ok(!pkg.is_empty() && applist::set_game(pkg, true))
    }

    fn removeGame(&self, pkg: &str) -> BinderResult<bool> {
        Ok(!pkg.is_empty() && applist::set_game(pkg, false))
    }

    fn getGames(&self) -> BinderResult<String> {
        Ok(applist::games().join("\n"))
    }

    fn getState(&self) -> BinderResult<String> {
        Ok(std::fs::read_to_string(STATE_FILE).unwrap_or_default())
    }
}

/// Registers the service and starts rsbinder's thread pool. Called after the
/// daemon's own libbinder_ndk pool is up: the two use separate binder fds and
/// separate thread pools, so rsbinder needs its own or incoming transactions to
/// this service are never dispatched.
///
/// init_default() must come first — both add_service and start_thread_pool route
/// through ProcessState::self(), which aborts with "ProcessState is not
/// initialized!" otherwise. Only start_thread_pool, not join_thread_pool: the
/// calling thread must stay free because main() joins the libbinder_ndk pool last.
pub(crate) fn start_binder_server() {
    rsbinder::ProcessState::init_default()
        .expect("Failed to initialize binder ProcessState");

    let binder = BnAZenithService::new_binder(AZenithService);
    match hub::add_service(SERVICE_NAME, binder.as_binder()) {
        Ok(()) => info!(comp::DAEMON, "aidl: service registered as {SERVICE_NAME}"),
        Err(e) => error!(comp::DAEMON, "aidl: add_service failed: {e}"),
    }
    rsbinder::ProcessState::start_thread_pool();
}
