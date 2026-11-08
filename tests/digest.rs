use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Iri, Kernel, Request, Verb};
use std::sync::Arc;

#[test]
fn digest_is_available_in_the_mounted_space() {
    let kernel = Kernel::new(Arc::new(ikigai_fn::space()));
    let request = Request::new(Verb::Source, Iri::parse("urn:iki:fn:digest").unwrap())
        .with_arg("in", ArgRef::Inline(b"abc".to_vec()));
    let output = block_on(kernel.issue(request, &Capability::root())).unwrap();
    assert_eq!(
        output.bytes,
        b"b3:6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
    );
}

#[test]
fn shared_vectors_match_core_and_the_cacheable_endpoint() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("vectors/digest_vectors.json")).unwrap();
    let kernel = Kernel::new(Arc::new(ikigai_fn::space()));
    for vector in vectors["vectors"].as_array().unwrap() {
        let input = vector["in"].as_str().unwrap();
        let expected = vector["digest"].as_str().unwrap();
        assert_eq!(
            ikigai_core::ContentId::of(input.as_bytes()).to_string(),
            expected
        );
        let request = Request::new(Verb::Source, Iri::parse("urn:iki:fn:digest").unwrap())
            .with_arg("in", ArgRef::Inline(input.as_bytes().to_vec()));
        let output = block_on(kernel.issue(request.clone(), &Capability::root())).unwrap();
        assert_eq!(output.bytes, expected.as_bytes(), "{input:?}");
        assert_eq!(output.expiry, ikigai_core::Expiry::Never);
        assert_eq!(output.repr_type.media_type, "text/plain");
        assert_eq!(output.repr_type.params.get("charset").unwrap(), "utf-8");
        assert!(kernel.is_cached(&request, &Capability::root()));
    }
}

#[test]
fn missing_and_non_utf8_inputs_are_typed_errors() {
    let kernel = Kernel::new(Arc::new(ikigai_fn::space()));
    let request = Request::new(Verb::Source, Iri::parse("urn:iki:fn:digest").unwrap());
    assert!(matches!(
        block_on(kernel.issue(request.clone(), &Capability::root())),
        Err(ikigai_core::Error::MissingArgument(name)) if name == "in"
    ));
    assert!(matches!(
        block_on(kernel.issue(request.with_arg("in", ArgRef::Inline(vec![0xff])), &Capability::root())),
        Err(ikigai_core::Error::InvalidArgument { name, .. }) if name == "in"
    ));
}

#[test]
fn composition_can_digest_a_referenced_value() {
    use ikigai_core::{Exact, FnEndpoint, Invocation, ReprType, Representation};
    let space = ikigai_fn::space()
        .bind(
            Exact::new("urn:test:text"),
            FnEndpoint::new("text", |_: &Invocation<'_>| {
                Ok(Representation::new(ReprType::new("text/plain"), b"abc".to_vec()).cacheable())
            }),
        )
        .bind(
            Exact::new("urn:test:template"),
            FnEndpoint::new("template", |_: &Invocation<'_>| {
                Ok(Representation::new(
                    ReprType::new("text/plain"),
                    b"$h{urn:iki:fn:digest?in=<urn:test:text>}".to_vec(),
                )
                .cacheable())
            }),
        );
    let kernel = Kernel::new(Arc::new(space));
    let request = Request::new(Verb::Source, Iri::parse("urn:iki:fn:compose").unwrap())
        .with_arg("src", ArgRef::Inline(b"urn:test:template".to_vec()));
    let output = block_on(kernel.issue(request, &Capability::root())).unwrap();
    assert_eq!(
        output.bytes,
        b"b3:6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
    );
}
