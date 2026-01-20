// Generate bindings from WIT
wit_bindgen::generate!({
    world: "service",
    path: "../wit",
    generate_all,
    additional_derives: [serde::Serialize, serde::Deserialize],
});
