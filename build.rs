fn main() {
    println!("cargo:rerun-if-changed=docs/images/logo.ico");
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rustc-env=AGENT_PUBLISHER=Jun Mystery");
    println!("cargo:rustc-env=AGENT_AUTHOR_EMAIL=darkzeuslk@gmail.com");

    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=AGENT_TARGET_TRIPLE={}", target);

    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8(o.stdout).ok() } else { None })
        .unwrap_or_else(|| "release".to_string());
    println!("cargo:rustc-env=AGENT_GIT_COMMIT={}", git_hash.trim());

    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-arg=-Wl,--build-id");
    }

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("docs/images/logo.ico");
        res.set("FileDescription", "Agent Guidance MCP Server & GraphRAG Intelligence Engine");
        res.set("ProductName", "Agent Guidance");
        res.set("CompanyName", "Jun Mystery");
        res.set("LegalCopyright", "Copyright © 2026 Jun Mystery. All rights reserved.");
        res.set("OriginalFilename", "agent-guidance.exe");
        res.set("InternalName", "agent-guidance");

        const WINDOWS_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="JunMystery.AgentGuidance" version="1.5.9.0" processorArchitecture="*"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}"/>
      <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>"#;
        res.set_manifest(WINDOWS_MANIFEST);

        if std::process::Command::new("rc.exe").arg("/?").output().is_err() {
            let kit_dirs = [
                r"E:\Windows Kits\10\bin\10.0.28000.0\x64",
                r"E:\Windows Kits\10\bin\10.0.26100.0\x64",
                r"E:\Windows Kits\10\bin\10.0.22621.0\x64",
                r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64",
                r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64",
            ];
            for dir in kit_dirs {
                if std::path::Path::new(dir).join("rc.exe").exists() {
                    res.set_toolkit_path(dir);
                    break;
                }
            }
        }

        if let Err(e) = res.compile() {
            println!("cargo:warning=winres icon compilation error: {}", e);
        }
    }
}
