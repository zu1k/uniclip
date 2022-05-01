fn main() {
    prost_build::compile_protos(&["src/msg.proto"], &["src/"]).unwrap();
}
