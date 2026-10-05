// Capture build inputs rather than reading a mutable compiler checkout at runtime.
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, env, fs, path::Path};

fn collect(root: &Path, path: &Path, files: &mut BTreeMap<String, String>) {
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read compiler input directory") {
            collect(root, &entry.expect("compiler input entry").path(), files);
        }
    } else {
        let name = path
            .strip_prefix(root)
            .expect("workspace input")
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(path).expect("read compiler input");
        files.insert(name, format!("sha256:{:x}", Sha256::digest(bytes)));
    }
}

fn main() {
    let manifest = std::path::PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.ancestors().nth(2).expect("compiler workspace");
    let mut files = BTreeMap::new();
    for path in ["Cargo.toml", "Cargo.lock", "data"] {
        collect(root, &root.join(path), &mut files);
    }
    // Include the CLI as an input to the local compiler distribution, although
    // library hosts may not link it. No executable/issuer attestation is claimed.
    for name in ["core", "cli", "syntax", "semantic", "diagnostics"] {
        let krate = root.join("crates").join(name);
        // Watch the existing directory to notice a newly added build script.
        // Watching an absent build.rs makes Cargo rebuild on every invocation.
        println!("cargo:rerun-if-changed={}", krate.display());
        collect(root, &krate.join("Cargo.toml"), &mut files);
        collect(root, &krate.join("src"), &mut files);
        let build = krate.join("build.rs");
        if build.is_file() {
            collect(root, &build, &mut files);
        }
    }
    let bytes = serde_json::to_vec(&files).expect("serialize compiler input manifest");
    let value = serde_json::json!({
        "status": "build_time_source_manifest",
        "manifest_digest": format!("sha256:{:x}", Sha256::digest(&bytes)),
        "files": files,
        "binary_attestation": null,
        "limitations": ["Source inputs and dependency lock only; not a toolchain, executable or protected-build attestation."]
    });
    let out = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(
        out.join("approval-compiler-inputs.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .expect("write compiler input provenance");
}
