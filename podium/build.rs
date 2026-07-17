fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc not found");
    std::env::set_var("PROTOC", protoc);
    tonic_build::configure()
        .build_server(false)
        .compile_protos(&["proto/maestro_android.proto"], &["proto"])
        .expect("tonic_build failed");
}
