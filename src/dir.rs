use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use nix::sys::stat::Mode;
use nix::unistd::Gid;
use nix::unistd::Uid;
use walkdir::WalkDir;

use crate::entry::Directory;
use crate::File;
use crate::Filesystem;

impl<'f> Filesystem<'f, 'f> {
    /// Create an in-memory view of a Filesystem from a directory on a real,
    /// on-disk filesystem.
    pub fn from_dir(path: &Path) -> std::io::Result<Self> {
        let mut fs = Self::new();
        for entry in WalkDir::new(path) {
            let entry = entry?;
            let relpath = entry
                .path()
                .strip_prefix(path)
                .expect("symlink traversal is disabled, this path must be under the top directory")
                .to_path_buf();
            let meta = entry.metadata()?;
            let mut xattrs = BTreeMap::new();
            for name in xattr::list(entry.path())? {
                let val = xattr::get(entry.path(), &name)?.expect("must exist");
                xattrs.insert(name, val);
            }
            let xattrs = xattrs
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect::<BTreeMap<Cow<'f, OsStr>, Cow<'f, [u8]>>>();
            if entry.file_type().is_dir() {
                fs.entries.insert(
                    relpath.into(),
                    Directory::builder()
                        .mode(Mode::from_bits_truncate(meta.mode()))
                        .uid(Uid::from_raw(meta.uid()))
                        .gid(Gid::from_raw(meta.gid()))
                        .xattrs(xattrs)
                        .build()
                        .into(),
                );
            } else if entry.file_type().is_symlink() {
                todo!()
            } else if entry.file_type().is_file() {
                fs.entries.insert(
                    relpath.into(),
                    File::builder()
                        .contents(std::fs::read(entry.path())?)
                        .mode(Mode::from_bits_truncate(meta.mode()))
                        .uid(Uid::from_raw(meta.uid()))
                        .gid(Gid::from_raw(meta.gid()))
                        .xattrs(xattrs)
                        .build()
                        .into(),
                );
            }
        }
        Ok(fs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::demo_fs;

    #[test]
    fn from_dir() {
        let fs = Filesystem::from_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/fs"))
            .expect("failed to load from directory");
        assert_eq!(fs, demo_fs());
    }
}