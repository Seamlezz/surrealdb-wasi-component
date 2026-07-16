wit_bindgen::generate!({
    path: "wit",
    world: "component",
    generate_all,
});

pub(crate) fn current_parent_context() -> Option<wasmcloud::observability::propagation::TraceContext>
{
    parent_context(otel_wasi::current_propagation_context())
}

fn parent_context(
    context: Option<otel_wasi::PropagationContext>,
) -> Option<wasmcloud::observability::propagation::TraceContext> {
    context.map(
        |context| wasmcloud::observability::propagation::TraceContext {
            traceparent: context.traceparent,
            tracestate: context.tracestate,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::parent_context;

    #[test]
    fn query_subscribe_and_cancel_context_is_passed_without_changes() {
        let expected_traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        let context = || {
            parent_context(Some(otel_wasi::PropagationContext {
                traceparent: expected_traceparent.into(),
                tracestate: Some("vendor=value".into()),
            }))
            .unwrap()
        };

        for operation_context in [context(), context(), context()] {
            assert_eq!(operation_context.traceparent, expected_traceparent);
            assert_eq!(
                operation_context.tracestate.as_deref(),
                Some("vendor=value")
            );
        }
    }

    #[test]
    fn absent_context_is_passed_as_none() {
        assert!(parent_context(None).is_none());
    }
}
