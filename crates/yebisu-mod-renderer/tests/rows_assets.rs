//! Rows and assets against the adopted copy, at list, copy, host and
//! application scope with negotiated values.
mod common;
use common::*;
use yebisu_mod_renderer::*;
use yebisu_mod_wire as wire;
use yebisu_mod_wire::{Counter, EntityKind, EntityRef};

fn adopted(
    name: &str,
    n: u8,
    limits: &wire::NegotiatedLimits,
    ledgers: &mut Ledgers,
    lists: &[&str],
) -> CopyReceiver {
    let mut receiver = receiver(name, n, limits.clone());
    let bytes = publish_bytes(receiver.context(), publish(1, 1, lists));
    receiver.adopt_bytes(&bytes, &mut ledgers.scopes()).unwrap();
    receiver
}

#[test]
fn row_requests_need_a_list_node_of_the_adopted_copy() {
    let limits = wire::NegotiatedLimits::default();
    let mut ledgers = Ledgers::new(&limits);
    let mut empty = receiver("a", 3, limits.clone());
    assert!(matches!(
        empty.request_rows(rows_request("r", "list", 1, 2), &mut ledgers.scopes()),
        Err(Error::NotAdopted)
    ));
    let mut receiver = adopted("a", 3, &limits, &mut ledgers, &["list"]);
    for request in [
        rows_request("r1", "missing", 1, 2),
        rows_request("r2", "list", 2, 2),
    ] {
        assert!(matches!(
            receiver.request_rows(request, &mut ledgers.scopes()),
            Err(Error::NotInAdoptedCopy("list"))
        ));
    }
    receiver
        .request_rows(rows_request("r3", "list", 1, 2), &mut ledgers.scopes())
        .unwrap();
    assert!(matches!(
        receiver.request_rows(rows_request("r3", "list", 1, 2), &mut ledgers.scopes()),
        Err(Error::DuplicateRequest(_))
    ));
}

#[test]
fn row_requests_stop_at_the_negotiated_list_copy_and_application_values() {
    let limits = lowered(|l| {
        l.rows = 5;
        l.row_requests_list = 1;
        l.row_requests_copy = 2;
        l.row_requests_app = 3;
    });
    let mut ledgers = Ledgers::new(&limits);
    let mut a = adopted("a", 3, &limits, &mut ledgers, &["x", "y", "z"]);
    let mut b = adopted("b", 4, &limits, &mut ledgers, &["x", "y"]);
    // Above the negotiated row count, below the A8 ceiling of 200.
    assert!(
        a.request_rows(rows_request("big", "x", 1, 6), &mut ledgers.scopes())
            .is_err()
    );
    a.request_rows(rows_request("a1", "x", 1, 1), &mut ledgers.scopes())
        .unwrap();
    // List scope: one request per list.
    assert!(
        a.request_rows(rows_request("a2", "x", 1, 1), &mut ledgers.scopes())
            .is_err()
    );
    a.request_rows(rows_request("a3", "y", 1, 1), &mut ledgers.scopes())
        .unwrap();
    // Copy scope: two per copy, though list z is free.
    assert!(
        a.request_rows(rows_request("a4", "z", 1, 1), &mut ledgers.scopes())
            .is_err()
    );
    // Application scope: three across both copies.
    b.request_rows(rows_request("b1", "x", 1, 1), &mut ledgers.scopes())
        .unwrap();
    assert!(
        b.request_rows(rows_request("b2", "y", 1, 1), &mut ledgers.scopes())
            .is_err()
    );
    // Refused requests left every ledger unchanged.
    assert_eq!(ledgers.app.used(wire::Resource::RowRequestsApp), 3);
    assert_eq!(a.outstanding_rows(), 2);
    a.cancel_rows("a1", &mut ledgers.scopes()).unwrap();
    b.request_rows(rows_request("b2", "y", 1, 1), &mut ledgers.scopes())
        .unwrap();
}

#[test]
fn row_replies_are_authenticated_matched_and_checked_against_the_adopted_copy() {
    let limits = wire::NegotiatedLimits::default();
    let mut ledgers = Ledgers::new(&limits);
    let mut receiver = adopted("a", 3, &limits, &mut ledgers, &["list"]);
    let request = rows_request("r", "list", 1, 2);
    receiver
        .request_rows(request.clone(), &mut ledgers.scopes())
        .unwrap();
    let reply = rows_reply(&request, None);
    // Another copy's envelope is rejected before the request is consulted.
    let foreign = envelope(&context("b", 4), 5, reply.clone());
    assert!(matches!(
        receiver.receive_rows(&foreign, &mut ledgers.scopes()),
        Err(Error::Wire(wire::Error::Stale(_)))
    ));
    let own = receiver.context().clone();
    let unknown = rows_reply(&rows_request("other", "list", 1, 2), None);
    assert!(matches!(
        receiver.receive_rows(&envelope(&own, 6, unknown), &mut ledgers.scopes()),
        Err(Error::UnknownRequest(_))
    ));
    // A row bound to an entity the adopted copy does not have.
    let stranger = EntityRef {
        id: Counter(99),
        incarnation: id(3),
        kind: EntityKind::Input,
    };
    let bad = rows_reply(&request, Some(stranger));
    assert!(
        receiver
            .receive_rows(&envelope(&own, 7, bad), &mut ledgers.scopes())
            .is_err()
    );
    assert_eq!(receiver.outstanding_rows(), 1);
    let rows = receiver
        .receive_rows(&envelope(&own, 8, reply), &mut ledgers.scopes())
        .unwrap();
    assert_eq!(rows.rows.len(), 2);
    assert_eq!(receiver.outstanding_rows(), 0);
    assert_eq!(ledgers.app.used(wire::Resource::RowRequestsApp), 0);
}

#[test]
fn row_replies_use_the_negotiated_reply_limits() {
    // The reply is a few hundred bytes: under the 256 KiB ceiling, over this value.
    let limits = lowered(|l| l.rows_bytes = 100);
    let mut ledgers = Ledgers::new(&limits);
    let mut receiver = adopted("a", 3, &limits, &mut ledgers, &["list"]);
    let request = rows_request("r", "list", 1, 2);
    receiver
        .request_rows(request.clone(), &mut ledgers.scopes())
        .unwrap();
    let own = receiver.context().clone();
    let reply = rows_reply(&request, None);
    let err = receiver
        .receive_rows(&envelope(&own, 3, reply), &mut ledgers.scopes())
        .unwrap_err();
    assert!(matches!(err, Error::Wire(wire::Error::Limit(_))), "{err:?}");
}

#[test]
fn adopting_a_copy_without_the_list_ends_its_requests_and_returns_reservations() {
    let limits = wire::NegotiatedLimits::default();
    let mut ledgers = Ledgers::new(&limits);
    let mut receiver = adopted("a", 3, &limits, &mut ledgers, &["list"]);
    receiver
        .request_rows(rows_request("r", "list", 1, 2), &mut ledgers.scopes())
        .unwrap();
    receiver
        .request_asset(asset_request("asset"), &mut ledgers.scopes())
        .unwrap();
    let mut next = publish(2, 2, &[]);
    next.asset_manifests.clear();
    next.views_full[0].nodes.truncate(1);
    next.views_full[0].nodes[0].children.clear();
    next.entities_full.truncate(1);
    next.validate_shape().unwrap();
    let bytes = publish_bytes(receiver.context(), next);
    receiver.adopt_bytes(&bytes, &mut ledgers.scopes()).unwrap();
    assert_eq!(receiver.outstanding_rows(), 0);
    assert_eq!(receiver.outstanding_assets(), 0);
    assert_eq!(ledgers.app.used(wire::Resource::RowRequestsApp), 0);
    assert_eq!(ledgers.host.used(wire::Resource::AssetTransfersHost), 0);
    assert_eq!(ledgers.app.used(wire::Resource::AssetTransfersApp), 0);
}

fn deliver(receiver: &mut CopyReceiver, id: &str, bytes: &[u8]) -> Result<()> {
    let own = receiver.context().clone();
    let reply = wire::AssetReply {
        request_id: id.into(),
        asset: asset_request(id).asset,
        channel: 7,
        total_bytes: SVG.len(),
        chunk_count: 1,
    };
    receiver.begin_asset(&envelope(&own, 3, reply))?;
    let frame = wire::DataChunk {
        index: 0,
        bytes: bytes.to_vec(),
    }
    .to_frame(7)?;
    receiver.push_asset_frame(id, &frame)
}

#[test]
fn asset_transfers_stop_at_the_negotiated_copy_host_and_application_values() {
    // Each scope's value is distinct, so each refusal below names one scope.
    let limits = lowered(|l| {
        l.asset_transfers_copy = 1;
        l.asset_transfers_host = 2;
        l.asset_transfers_app = 3;
    });
    let mut host_a = Ledgers::new(&limits);
    let mut host_b = wire::ResourceLedger::negotiated(&limits).unwrap();
    let mut a = adopted("a", 3, &limits, &mut host_a, &["list"]);
    let mut b = adopted("b", 4, &limits, &mut host_a, &["list"]);
    // A second connection to copy "a" of the same host.
    let mut c = adopted("a", 3, &limits, &mut host_a, &["list"]);
    let mut d = adopted("b", 4, &limits, &mut host_a, &["list"]);
    a.request_asset(asset_request("a1"), &mut host_a.scopes())
        .unwrap();
    // Copy scope: one transfer per copy.
    assert!(
        a.request_asset(asset_request("a2"), &mut host_a.scopes())
            .is_err()
    );
    b.request_asset(asset_request("b1"), &mut host_a.scopes())
        .unwrap();
    // Host scope: two transfers for every copy and connection of this host.
    assert!(
        c.request_asset(asset_request("c1"), &mut host_a.scopes())
            .is_err()
    );
    // A second host has room of its own and takes the third application unit.
    let mut other_host = SharedScopes {
        host: &mut host_b,
        app: &mut host_a.app,
    };
    c.request_asset(asset_request("c1"), &mut other_host)
        .unwrap();
    // Application scope: three across both hosts.
    assert!(
        d.request_asset(asset_request("d1"), &mut other_host)
            .is_err()
    );
    // Refused requests left every ledger unchanged.
    assert_eq!(host_a.host.used(wire::Resource::AssetTransfersHost), 2);
    assert_eq!(host_b.used(wire::Resource::AssetTransfersHost), 1);
    assert_eq!(host_a.app.used(wire::Resource::AssetTransfersApp), 3);
    // Completing a transfer returns its units at every scope.
    deliver(&mut a, "a1", SVG).unwrap();
    assert_eq!(a.finish_asset("a1", &mut host_a.scopes()).unwrap(), SVG);
    assert_eq!(host_a.host.used(wire::Resource::AssetTransfersHost), 1);
    a.request_asset(asset_request("a3"), &mut host_a.scopes())
        .unwrap();
}

#[test]
fn asset_replies_are_authenticated_matched_and_digest_checked() {
    let limits = wire::NegotiatedLimits::default();
    let mut ledgers = Ledgers::new(&limits);
    let mut receiver = adopted("a", 3, &limits, &mut ledgers, &["list"]);
    assert!(matches!(
        deliver(&mut receiver, "never-asked", SVG),
        Err(Error::UnknownRequest(_))
    ));
    receiver
        .request_asset(asset_request("x"), &mut ledgers.scopes())
        .unwrap();
    let foreign = wire::AssetReply {
        request_id: "x".into(),
        asset: asset_request("x").asset,
        channel: 7,
        total_bytes: SVG.len(),
        chunk_count: 1,
    };
    assert!(matches!(
        receiver.begin_asset(&envelope(&context("b", 4), 2, foreign)),
        Err(Error::Wire(wire::Error::Stale(_)))
    ));
    let mut tampered = SVG.to_vec();
    tampered[0] = b' ';
    deliver(&mut receiver, "x", &tampered).unwrap();
    assert!(matches!(
        receiver.finish_asset("x", &mut ledgers.scopes()),
        Err(Error::Wire(wire::Error::DigestMismatch))
    ));
    assert_eq!(ledgers.host.used(wire::Resource::AssetTransfersHost), 0);
}

#[test]
fn an_asset_over_the_negotiated_size_is_refused_with_its_publication() {
    // The manifest is checked at adoption, so no request for it can exist.
    let limits = lowered(|l| l.svg_bytes = SVG.len() - 1);
    let mut ledgers = Ledgers::new(&limits);
    let mut receiver = receiver("a", 3, limits.clone());
    let bytes = publish_bytes(receiver.context(), publish(1, 1, &["list"]));
    assert!(matches!(
        receiver.adopt_bytes(&bytes, &mut ledgers.scopes()),
        Err(Error::Wire(wire::Error::Limit(_)))
    ));
    assert!(matches!(
        receiver.request_asset(asset_request("x"), &mut ledgers.scopes()),
        Err(Error::NotAdopted)
    ));
}
