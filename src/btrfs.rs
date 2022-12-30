use std::borrow::Borrow;
use std::collections::BTreeMap;

use sendstream_parser::Command;
use sendstream_parser::Sendstream;
use uuid::Uuid;

use crate::entry::Directory;
use crate::file::File;
use crate::Filesystem;
use crate::Result;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invariant violated: {0}")]
    InvariantViolated(&'static str),
    #[error("parent subvol not yet received: {0}")]
    MissingParent(Uuid),
    #[error(transparent)]
    Parse(#[from] sendstream_parser::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subvol {
    parent_uuid: Option<Uuid>,
    fs: Filesystem,
}

impl Subvol {
    fn new() -> Self {
        Subvol {
            parent_uuid: None,
            fs: Filesystem::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subvols(BTreeMap<Uuid, Subvol>);

impl Subvols {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Parse subvolumes from an uncompressed sendstream
    pub fn receive<'f>(&mut self, sendstream: Sendstream<'f>) -> Result<()> {
        let mut cmd_iter = sendstream.commands().iter();
        let (subvol_uuid, mut subvol) = match cmd_iter
            .next()
            .expect("must have at least one command")
        {
            Command::Subvol(s) => {
                let mut subvol = Subvol::new();
                subvol.fs.insert("", Directory::default());
                (s.uuid(), subvol)
            }
            Command::Snapshot(s) => {
                let mut subvol = self
                    .0
                    .get(&s.clone_uuid())
                    .ok_or(Error::MissingParent(s.clone_uuid()))?
                    .clone();
                subvol.parent_uuid = Some(s.clone_uuid());
                (s.uuid(), subvol)
            }
            _ => return Err(Error::InvariantViolated("first command was not subvol start").into()),
        };
        for cmd in cmd_iter {
            match cmd {
                Command::Chmod(c) => {
                    subvol.fs.chmod(c.path().borrow(), c.mode().mode())?;
                }
                _ => eprintln!("unimplemented command: {:?}", cmd),
            }
        }
        self.0.insert(subvol_uuid, subvol);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use bytes::Bytes;
    use pretty_assertions::assert_eq;
    use uuid::uuid;

    use super::*;
    use crate::tests::demo_fs;

    #[test]
    fn sendstream() {
        let contents = Bytes::from(
            std::fs::read(Path::new(env!("OUT_DIR")).join("testdata.sendstream"))
                .expect("failed to read testdata.sendstream"),
        );
        let sendstreams = Sendstream::parse_all(&contents).expect("failed to parse sendstream");
        let mut subvols = Subvols::new();
        for sendstream in sendstreams {
            subvols
                .receive(sendstream)
                .expect("failed to receive sendstream");
        }
        assert_eq!(
            BTreeMap::from([
                (
                    uuid!("0fbf2b5f-ff82-a748-8b41-e35aec190b49"),
                    Subvol {
                        parent_uuid: None,
                        fs: demo_fs(),
                    }
                ),
                (
                    uuid!("ed2c87d3-12e3-c549-a699-635de66d6f35"),
                    Subvol {
                        parent_uuid: Some(uuid!("0fbf2b5f-ff82-a748-8b41-e35aec190b49")),
                        fs: demo_fs(),
                    }
                )
            ]),
            subvols.0
        );
    }
}
