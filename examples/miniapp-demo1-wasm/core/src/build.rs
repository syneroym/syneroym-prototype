fn main() {
    println!("cargo:rerun-if-changed=../wit");

    // Generate host bindings from WIT
    wasmtime::component::bindgen!({
        world: "service",
        path: "../wit",
        async: true,
        tracing: true,
    });
}
