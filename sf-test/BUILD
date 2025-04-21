load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")
load("@crate_index//:defs.bzl", "aliases", "all_crate_deps")

package(default_visibility = ["//visibility:public"])

rust_library(
    name = "sf-test",
    srcs = glob(["src/**/*.rs"]),
    crate_name = "sf_test",
    deps = all_crate_deps(
        normal = True,
    ),
)

rust_test(
    name = "sf-test_test",
    crate = ":sf-test",
    deps = all_crate_deps(
        normal = True,
        normal_dev = True,
    ),
)

filegroup(
    name = "cargo_toml",
    srcs = ["Cargo.toml"],
    visibility = ["//visibility:public"],
) 
