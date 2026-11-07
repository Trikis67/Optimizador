fn main() {
    println!("cargo:rerun-if-changed=admin-manifest.xml");
    // Solo la versión final pide administrador al abrirse. En `npm run dev` no, porque
    // `tauri dev` no podría lanzar la app sin una consola elevada (error 740).
    let release = std::env::var("PROFILE").map(|p| p == "release").unwrap_or(false);
    let mut windows = tauri_build::WindowsAttributes::new();
    if release {
        let manifest = include_str!("admin-manifest.xml");
        // El compilador de recursos no guarda bien los acentos: un solo carácter no ASCII deja el
        // manifiesto inválido y Windows se niega a abrir la app («Sintaxis XML no válida»).
        assert!(manifest.is_ascii(), "admin-manifest.xml solo puede tener caracteres ASCII");
        windows = windows.app_manifest(manifest);
    }
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("error al preparar la compilación");
}
