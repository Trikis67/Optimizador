//! Analizador de disco: recorre carpetas en paralelo y guarda el árbol en memoria.

use rayon::prelude::*;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

/// Un archivo o carpeta dentro del árbol escaneado.
pub struct Node {
    pub name: Box<str>,
    pub size: u64,
    pub files: u64,
    pub is_dir: bool,
    pub children: Vec<Node>,
}

/// Contadores compartidos entre los hilos del escaneo.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub cancel: AtomicBool,
}

impl Progress {
    pub fn reset(&self) {
        self.files.store(0, Relaxed);
        self.dirs.store(0, Relaxed);
        self.bytes.store(0, Relaxed);
        self.errors.store(0, Relaxed);
        self.cancel.store(false, Relaxed);
    }
}

/// Resultado completo de un escaneo.
pub struct ScanResult {
    pub root_path: PathBuf,
    pub root: Node,
}

/// Escanea una carpeta de forma recursiva. Las subcarpetas se recorren en paralelo.
pub fn scan_dir(path: &Path, name: Box<str>, p: &Progress) -> Node {
    let mut node = Node { name, size: 0, files: 0, is_dir: true, children: Vec::new() };
    if p.cancel.load(Relaxed) {
        return node;
    }
    p.dirs.fetch_add(1, Relaxed);

    let rd = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => {
            // Carpetas protegidas (System Volume Information, otros usuarios, etc.)
            p.errors.fetch_add(1, Relaxed);
            return node;
        }
    };

    let mut subdirs: Vec<(PathBuf, Box<str>)> = Vec::new();
    for entry in rd.flatten() {
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        // Saltamos enlaces simbólicos y "junctions" para no contar dos veces ni entrar en bucles.
        if ft.is_symlink() {
            continue;
        }
        let child_name: Box<str> = entry.file_name().to_string_lossy().into();
        if ft.is_dir() {
            subdirs.push((entry.path(), child_name));
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            p.files.fetch_add(1, Relaxed);
            p.bytes.fetch_add(size, Relaxed);
            node.size += size;
            node.files += 1;
            node.children.push(Node { name: child_name, size, files: 1, is_dir: false, children: Vec::new() });
        }
    }

    let dirs: Vec<Node> = subdirs
        .into_par_iter()
        .map(|(child_path, child_name)| scan_dir(&child_path, child_name, p))
        .collect();
    for d in dirs {
        node.size += d.size;
        node.files += d.files;
        node.children.push(d);
    }

    node.children.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    node.children.shrink_to_fit();
    node
}

/// Busca un nodo siguiendo una lista de nombres desde la raíz.
pub fn find<'a>(root: &'a Node, segments: &[String]) -> Option<&'a Node> {
    let mut cur = root;
    for seg in segments {
        cur = cur.children.iter().find(|c| c.is_dir && &*c.name == seg.as_str())?;
    }
    Some(cur)
}

/// Quita un nodo del árbol y descuenta su tamaño de todas las carpetas padre.
/// Devuelve (bytes, archivos) eliminados.
pub fn remove(node: &mut Node, segments: &[String]) -> Option<(u64, u64)> {
    let (first, rest) = segments.split_first()?;
    let idx = node.children.iter().position(|c| &*c.name == first.as_str())?;
    let removed = if rest.is_empty() {
        let c = node.children.remove(idx);
        (c.size, c.files)
    } else {
        remove(&mut node.children[idx], rest)?
    };
    node.size = node.size.saturating_sub(removed.0);
    node.files = node.files.saturating_sub(removed.1);
    if !rest.is_empty() {
        node.children.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    }
    Some(removed)
}

#[derive(Serialize)]
pub struct Item {
    pub name: String,
    pub size: u64,
    pub files: u64,
    pub is_dir: bool,
    pub has_children: bool,
}

#[derive(Serialize)]
pub struct Listing {
    pub path: String,
    pub size: u64,
    pub files: u64,
    pub total_items: usize,
    pub items: Vec<Item>,
}

pub fn listing(root_path: &Path, node: &Node, segments: &[String], limit: usize) -> Listing {
    let mut full = root_path.to_path_buf();
    for s in segments {
        full.push(s);
    }
    Listing {
        path: full.to_string_lossy().into_owned(),
        size: node.size,
        files: node.files,
        total_items: node.children.len(),
        items: node
            .children
            .iter()
            .take(limit)
            .map(|c| Item {
                name: c.name.to_string(),
                size: c.size,
                files: c.files,
                is_dir: c.is_dir,
                has_children: !c.children.is_empty(),
            })
            .collect(),
    }
}

#[derive(Serialize)]
pub struct BigFile {
    pub path: String,
    pub segments: Vec<String>,
    pub size: u64,
}

/// Los N archivos más grandes de todo el árbol.
pub fn top_files(result: &ScanResult, limit: usize) -> Vec<BigFile> {
    // Montículo de mínimos con los N más grandes; guardamos la ruta como lista de índices.
    let mut heap: BinaryHeap<Reverse<(u64, Vec<u32>)>> = BinaryHeap::new();
    let mut stack: Vec<(&Node, Vec<u32>)> = vec![(&result.root, Vec::new())];
    while let Some((node, idx_path)) = stack.pop() {
        for (i, c) in node.children.iter().enumerate() {
            if c.is_dir {
                let mut p = idx_path.clone();
                p.push(i as u32);
                stack.push((c, p));
            } else if heap.len() < limit || heap.peek().map_or(true, |Reverse((s, _))| c.size > *s) {
                let mut p = idx_path.clone();
                p.push(i as u32);
                heap.push(Reverse((c.size, p)));
                if heap.len() > limit {
                    heap.pop();
                }
            }
        }
    }

    let mut out: Vec<BigFile> = heap
        .into_iter()
        .map(|Reverse((size, idx_path))| {
            let mut segments = Vec::with_capacity(idx_path.len());
            let mut cur = &result.root;
            for i in idx_path {
                cur = &cur.children[i as usize];
                segments.push(cur.name.to_string());
            }
            let mut full = result.root_path.clone();
            for s in &segments {
                full.push(s);
            }
            BigFile { path: full.to_string_lossy().into_owned(), segments, size }
        })
        .collect();
    out.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    out
}

#[derive(Serialize)]
pub struct ExtStat {
    pub ext: String,
    pub size: u64,
    pub count: u64,
}

/// Espacio usado por cada extensión de archivo.
pub fn ext_stats(root: &Node, limit: usize) -> Vec<ExtStat> {
    let mut map: HashMap<String, (u64, u64)> = HashMap::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        for c in &node.children {
            if c.is_dir {
                stack.push(c);
            } else {
                let ext = match c.name.rsplit_once('.') {
                    Some((base, e)) if !base.is_empty() && e.len() <= 10 => e.to_lowercase(),
                    _ => "(sin extensión)".to_string(),
                };
                let entry = map.entry(ext).or_insert((0, 0));
                entry.0 += c.size;
                entry.1 += 1;
            }
        }
    }
    let mut v: Vec<ExtStat> = map.into_iter().map(|(ext, (size, count))| ExtStat { ext, size, count }).collect();
    v.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    v.truncate(limit);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escanea_y_borra() {
        let dir = std::env::temp_dir().join("optimizador_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("a/b")).unwrap();
        fs::write(dir.join("a/b/grande.mp4"), vec![0u8; 5000]).unwrap();
        fs::write(dir.join("a/chico.txt"), vec![0u8; 100]).unwrap();
        fs::write(dir.join("raiz.zip"), vec![0u8; 1000]).unwrap();

        let p = Progress::default();
        let root = scan_dir(&dir, "test".into(), &p);
        assert_eq!(root.size, 6100);
        assert_eq!(root.files, 3);
        assert_eq!(&*root.children[0].name, "a");

        let res = ScanResult { root_path: dir.clone(), root };
        let top = top_files(&res, 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].size, 5000);
        assert_eq!(top[0].segments, vec!["a", "b", "grande.mp4"]);

        let exts = ext_stats(&res.root, 10);
        assert_eq!(exts[0].ext, "mp4");

        let mut res = res;
        let removed = remove(&mut res.root, &["a".into(), "b".into(), "grande.mp4".into()]).unwrap();
        assert_eq!(removed, (5000, 1));
        assert_eq!(res.root.size, 1100);
        assert_eq!(find(&res.root, &["a".into()]).unwrap().size, 100);
        // tras borrar, "raiz.zip" (1000) pasa a ser el más grande
        assert_eq!(&*res.root.children[0].name, "raiz.zip");
        let _ = fs::remove_dir_all(&dir);
    }
}
