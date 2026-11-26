//! Handshake acceptance and atomic publication adoption under negotiated limits.
mod common;
use common::*;
use yebisu_mod_renderer::*;
use yebisu_mod_wire as wire;
use yebisu_mod_wire::Counter;

fn handshake(limits: wire::NegotiatedLimits) -> ClientHandshake {
    ClientHandshake::new(CLIENT.into(), id(9), digests(), vec![], limits).unwrap()
}

#[test]
fn hello_ack_must_echo_the_nonce_and_not_raise_any_offered_limit() {
    let offered = lowered(|l| l.nodes = 1_000);
    let client = handshake(offered.clone());
    assert_eq!(client.hello().limits, offered);
    let session = client
        .accept(&ack(id(9), lowered(|l| l.nodes = 500)))
        .unwrap();
    assert_eq!(session.limits.nodes, 500);
    assert!(matches!(
        client.accept(&ack(id(8), offered.clone())),
        Err(Error::Wire(wire::Error::Stale(_)))
    ));
    assert!(matches!(
        client.accept(&ack(id(9), wire::NegotiatedLimits::default())),
        Err(Error::Wire(wire::Error::Limit(_)))
    ));
    let mut other_protocol = ack(id(9), offered);
    other_protocol.protocol = 2;
    assert!(client.accept(&other_protocol).is_err());
}

#[test]
fn receiving_contexts_exist_only_for_copies_the_ack_announced() {
    let session = negotiated(wire::NegotiatedLimits::default());
    let context = session
        .context(HOST.into(), CLIENT.into(), &copy("a", 3), Counter(1))
        .unwrap();
    assert_eq!(context.session.boot, id(1));
    assert_eq!(context.session.epoch, id(2));
    assert!(matches!(
        session.context(HOST.into(), CLIENT.into(), &copy("a", 7), Counter(1)),
        Err(Error::UnknownCopy)
    ));
}

fn transfer(
    receiver: &mut CopyReceiver,
    bytes: &[u8],
    ledgers: &mut Ledgers,
) -> Result<wire::Publish> {
    let context = receiver.context().clone();
    let chunks: Vec<_> = bytes.chunks(wire::DATA_CHUNK_BYTES).collect();
    let begin = wire::PublishBegin {
        total_bytes: bytes.len(),
        chunk_count: chunks.len(),
        digest: wire::bytes_digest(bytes),
        channel: 5,
    };
    receiver.begin_publish(&envelope(&context, 1, begin))?;
    for (i, chunk) in chunks.into_iter().enumerate() {
        let frame = wire::DataChunk {
            index: i as u32,
            bytes: chunk.to_vec(),
        }
        .to_frame(5)?;
        receiver.push_publish_frame(&frame)?;
    }
    let commit = wire::PublishCommit {
        digest: wire::bytes_digest(bytes),
    };
    Ok(receiver
        .commit_publish(
            &envelope(&context, 2, commit),
            &Uncompressed,
            &mut ledgers.scopes(),
        )?
        .clone())
}

#[test]
fn a_publication_transfer_is_adopted_whole_through_raw_chunks() {
    let limits = wire::NegotiatedLimits::default();
    let mut receiver = receiver("a", 3, limits.clone());
    let mut ledgers = Ledgers::new(&limits);
    let bytes = publish_bytes(receiver.context(), publish(1, 1, &["list"]));
    let adopted = transfer(&mut receiver, &bytes, &mut ledgers).unwrap();
    assert_eq!(adopted.pub_rev, Counter(1));
    assert_eq!(receiver.generation_high_watermark(), Counter(1));
}

#[test]
fn transfer_header_and_inflated_bytes_use_the_negotiated_values() {
    let probe = receiver("a", 3, wire::NegotiatedLimits::default());
    let bytes = publish_bytes(probe.context(), publish(1, 1, &["list"]));
    // Below the A8 ceiling, above the negotiated transfer size.
    let small = lowered(|l| l.publish_bytes = bytes.len() - 1);
    let mut receiver = receiver("a", 3, small.clone());
    let err = transfer(&mut receiver, &bytes, &mut Ledgers::new(&small)).unwrap_err();
    assert!(matches!(err, Error::Wire(wire::Error::Limit(_))), "{err:?}");
    assert!(receiver.adopted().is_none());
    let small = lowered(|l| l.decompressed_bytes = bytes.len() - 1);
    let mut receiver = receiver_with("a", 3, small.clone());
    let err = transfer(&mut receiver, &bytes, &mut Ledgers::new(&small)).unwrap_err();
    assert!(matches!(err, Error::Wire(wire::Error::Limit(_))), "{err:?}");
    assert!(receiver.adopted().is_none());
}
fn receiver_with(name: &str, n: u8, limits: wire::NegotiatedLimits) -> CopyReceiver {
    common::receiver(name, n, limits)
}

#[test]
fn a_publication_over_a_negotiated_limit_is_not_adopted_and_burns_no_generation() {
    let limits = lowered(|l| l.nodes = 3);
    let mut receiver = receiver("a", 3, limits.clone());
    let mut ledgers = Ledgers::new(&limits);
    // Root, SVG and two lists: four nodes, under the 20,000 ceiling.
    let over = publish_bytes(receiver.context(), publish(1, 1, &["x", "y"]));
    let err = receiver
        .adopt_bytes(&over, &mut ledgers.scopes())
        .unwrap_err();
    assert!(matches!(err, Error::Wire(wire::Error::Limit(_))), "{err:?}");
    assert!(receiver.adopted().is_none());
    assert_eq!(receiver.generation_high_watermark(), Counter(0));
    // The same generation is still available to an acceptable publication.
    let fits = publish_bytes(receiver.context(), publish(1, 1, &["x"]));
    receiver.adopt_bytes(&fits, &mut ledgers.scopes()).unwrap();
    assert_eq!(receiver.generation_high_watermark(), Counter(1));
}

#[test]
fn adoption_authenticates_the_envelope_and_keeps_generations_copy_wide() {
    let limits = wire::NegotiatedLimits::default();
    let mut receiver = receiver("a", 3, limits.clone());
    let mut ledgers = Ledgers::new(&limits);
    let other = context("b", 4);
    let foreign = publish_bytes(&other, publish(1, 1, &["list"]));
    assert!(matches!(
        receiver.adopt_bytes(&foreign, &mut ledgers.scopes()),
        Err(Error::Wire(wire::Error::Stale(_)))
    ));
    let first = publish_bytes(receiver.context(), publish(1, 1, &["list"]));
    receiver.adopt_bytes(&first, &mut ledgers.scopes()).unwrap();
    // A later publication may not reuse generation 1, even with a newer pub_rev.
    let reused = publish_bytes(receiver.context(), publish(2, 1, &["list"]));
    assert!(matches!(
        receiver.adopt_bytes(&reused, &mut ledgers.scopes()),
        Err(Error::Wire(wire::Error::Stale(_)))
    ));
    // Nor may an older pub_rev be displayed after a newer one.
    let next = publish_bytes(receiver.context(), publish(3, 2, &["list"]));
    receiver.adopt_bytes(&next, &mut ledgers.scopes()).unwrap();
    let older = publish_bytes(receiver.context(), publish(2, 3, &["list"]));
    assert!(receiver.adopt_bytes(&older, &mut ledgers.scopes()).is_err());
    assert_eq!(receiver.adopted().map(|p| p.pub_rev), Some(Counter(3)));
}

#[test]
fn control_messages_use_the_negotiated_size() {
    let receiver = receiver("a", 3, lowered(|l| l.control_bytes = 100));
    receiver.check_control(&[0; 100]).unwrap();
    assert!(receiver.check_control(&[0; 101]).is_err());
}
