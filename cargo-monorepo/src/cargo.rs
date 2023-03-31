//! Most of this file is direct copy of part of the
//! cargo-release source code, so kudos to them!
//! https://github.com/crate-ci/cargo-release
use anyhow::Context;
use cargo_metadata::{Metadata, PackageId};
use std::collections::{HashMap, HashSet};

pub fn sort_workspace(ws_meta: &Metadata) -> anyhow::Result<Vec<PackageId>> {
    let members: HashSet<_> = ws_meta.workspace_members.iter().collect();
    let dep_tree: HashMap<_, _> = ws_meta
        .resolve
        .as_ref()
        .with_context(|| "Failed to resolve workspace deps")?
        .nodes
        .iter()
        .filter_map(|n| {
            if members.contains(&n.id) {
                Some((&n.id, &n.dependencies))
            } else {
                None
            }
        })
        .collect();

    let mut sorted = Vec::new();
    let mut processed = HashSet::new();
    for pkg_id in ws_meta.workspace_members.iter() {
        sort_workspace_inner(ws_meta, pkg_id, &dep_tree, &mut processed, &mut sorted);
    }

    let sorted = sorted.into_iter().cloned().collect();

    Ok(sorted)
}

fn sort_workspace_inner<'m>(
    ws_meta: &'m Metadata,
    pkg_id: &'m PackageId,
    dep_tree: &HashMap<&'m PackageId, &'m Vec<PackageId>>,
    processed: &mut HashSet<&'m PackageId>,
    sorted: &mut Vec<&'m PackageId>,
) {
    if !processed.insert(pkg_id) {
        return;
    }

    for dep_id in dep_tree[pkg_id]
        .iter()
        .filter(|dep_id| dep_tree.contains_key(dep_id))
    {
        sort_workspace_inner(ws_meta, dep_id, dep_tree, processed, sorted);
    }

    sorted.push(pkg_id);
}
