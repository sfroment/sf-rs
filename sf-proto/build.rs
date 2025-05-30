use std::{fs, path::Path};

fn main() {
	let proto_dir = "./proto";
	let proto_files = vec!["identity.proto"];

	let out_dir = "src/proto";

	if !Path::new(out_dir).exists() {
		fs::create_dir_all(out_dir).expect("Failed to create output directory");
	}

	prost_build::Config::new()
		.out_dir("src/proto")
		.compile_protos(&proto_files, &[proto_dir])
		.expect("Failed to compile protobuf files");

	println!("cargo:rerun-if-changed={proto_files:?}");
}
