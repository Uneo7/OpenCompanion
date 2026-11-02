use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let ico = manifest.join("../../packaging/windows/opencompanion.ico");
    println!("cargo:rerun-if-changed={}", ico.display());
    println!("cargo:rerun-if-changed=build.rs");
    // The target, not the host: a Linux build made on Windows embeds nothing.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let num: Vec<u16> = version.split(['.', '-']).take(3).map(|p| p.parse().unwrap_or(0)).collect();
    let (a, b, c) = (num[0], num.get(1).copied().unwrap_or(0), num.get(2).copied().unwrap_or(0));
    let ico_path = ico.canonicalize().expect("packaging/windows/opencompanion.ico (run scripts/gen_icons)");
    // rc.exe wants a plain path (no `\\?\` verbatim prefix); forward slashes need no escaping.
    let plain = ico_path.display().to_string();
    let ico_rc = plain.strip_prefix(r"\\?\").unwrap_or(&plain).replace('\\', "/");
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{ico_rc}"
1 VERSIONINFO
FILEVERSION {a},{b},{c},0
PRODUCTVERSION {a},{b},{c},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "ProductName", "OpenCompanion"
      VALUE "FileDescription", "OpenCompanion"
      VALUE "CompanyName", "OpenCompanion contributors"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
      VALUE "OriginalFilename", "opencompanion.exe"
      VALUE "InternalName", "opencompanion"
      VALUE "LegalCopyright", "GPL-3.0-or-later, the OpenCompanion contributors"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("opencompanion.rc");
    std::fs::write(&out, rc).expect("writing opencompanion.rc");
    embed_resource::compile(&out, embed_resource::NONE).manifest_optional().expect("compiling the Windows resources");
}
