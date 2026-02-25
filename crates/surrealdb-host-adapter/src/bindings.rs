wasmtime::component::bindgen!({
    path: "wit",
    world: "adapter",
    imports: {
        default: async | trappable,
    },
});
