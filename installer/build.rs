//! Gives envbyte-setup.exe its icon, its name in Explorer and Task Manager, and
//! a manifest saying it runs as the user who opened it. Installing into the
//! user's own profile never needs administrator rights, so Windows must not
//! offer to elevate it just because "setup" is in its name.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=envbyte.ico");

    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("envbyte.ico")
            .set("FileDescription", "Envbyte Setup")
            .set("ProductName", "Envbyte")
            .set("OriginalFilename", "envbyte-setup.exe")
            .set_manifest(MANIFEST);
        if let Err(error) = resource.compile() {
            panic!("could not embed the Windows resources: {error}");
        }
    }
}

#[cfg(windows)]
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 and 11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
"#;
