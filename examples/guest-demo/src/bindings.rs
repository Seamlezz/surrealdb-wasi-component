use crate::GuestDemo;

wit_bindgen::generate!({
    path: "wit",
    world: "adapter",
    generate_all,
});

export!(GuestDemo);
