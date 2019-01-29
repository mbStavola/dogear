use std::collections::HashMap;

use error::{Error, ErrorKind};
use guid::Guid;
use merge::{Merger, Deletion};
use tree::{Content, MergedNode, Tree};

pub trait Store<E: From<Error>> {
    /// Builds a fully rooted, consistent tree from the items and tombstones in
    /// the local store.
    fn fetch_local_tree(&self, local_time_millis: i64) -> Result<Tree, E>;

    /// Fetches content info for all new local items that haven't been uploaded
    /// or merged yet. We'll try to dedupe them to remotely changed items with
    /// similar contents and different GUIDs.
    fn fetch_new_local_contents(&self) -> Result<HashMap<Guid, Content>, E>;

    /// Builds a fully rooted, consistent tree from the items and tombstones in
    /// the mirror.
    fn fetch_remote_tree(&self, remote_time_millis: i64) -> Result<Tree, E>;

    /// Fetches content info for all items in the mirror that changed since the
    /// last sync and don't exist locally. We'll try to match new local items to
    /// these.
    fn fetch_new_remote_contents(&self) -> Result<HashMap<Guid, Content>, E>;

    /// Applies the merged tree and stages items to upload. We keep this
    /// generic: on Desktop, we'll insert the merged tree into a temp
    /// table, update Places, and stage outgoing items in another temp
    /// table. Afterward, we can inflate records on the JS side. On mobile,
    /// this flow might be simpler.
    fn apply<D: Iterator<Item = Deletion>>(&mut self,
                                           merged_root: &MergedNode,
                                           deletions: D) -> Result<(), E>;

    fn merge(&mut self, local_time_millis: i64, remote_time_millis: i64) -> Result<(), E> {
        let local_tree = self.fetch_local_tree(local_time_millis)?;
        let new_local_contents = self.fetch_new_local_contents()?;

        let remote_tree = self.fetch_remote_tree(remote_time_millis)?;
        let new_remote_contents = self.fetch_new_remote_contents()?;

        let mut merger = Merger::with_contents(&local_tree, &new_local_contents,
                                               &remote_tree, &new_remote_contents);
        let merged_root = merger.merge()?;

        if !merger.subsumes(&local_tree) {
            Err(E::from(ErrorKind::UnmergedLocalItems.into()))?;
        }
        if !merger.subsumes(&remote_tree) {
            Err(E::from(ErrorKind::UnmergedRemoteItems.into()))?;
        }

        self.apply(&merged_root, merger.deletions())?;

        Ok(())
    }
}
