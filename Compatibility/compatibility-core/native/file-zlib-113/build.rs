// Integration additions; upstream source and copyright notices are unchanged.
pub fn compile(root: &std::path::Path) {
    let mut b=cc::Build::new();
    b.include(root).flag("-std=gnu89").flag("-UTARGET_OS_MAC")
      .flag("-Wno-deprecated-non-prototype").flag("-Wno-implicit-function-declaration")
      .flag("-Wno-implicit-int").flag("-include").flag(root.join("namespace.h").to_str().unwrap());
    for n in ["adler32.c","zutil.c","inflate.c","infblock.c","inftrees.c","infcodes.c","infutil.c","inffast.c"] {
        b.file(root.join(n));
    }
    for e in std::fs::read_dir(root).unwrap() {
        let p=e.unwrap().path(); if p.is_file(){println!("cargo:rerun-if-changed={}",p.display());}
    }
    b.warnings(false).compile("peggle_file_zlib113");
}
