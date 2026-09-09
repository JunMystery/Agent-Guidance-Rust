fn main() {
    println!("cargo:rerun-if-changed=docs/images/logo.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("docs/images/logo.ico");

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
