fn main() {
    prost_build::compile_protos(&["../../../grc20.proto"], &["../../../"])
        .expect("Failed to compile protos");
}
