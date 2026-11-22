//! `field-leak`: direct access across a boundary to a field of an encapsulated
//! struct (one that owns private invariants). Plain data structs (all fields
//! public) and return DTOs (mostly public fields with no `pub(crate)` hole)
//! are not flagged.

use std::collections::BTreeMap;

use super::{Diagnostic, Lint, Location, boundaries, clickable, home_boundary, in_scope};
use crate::model::{ItemKind, Model, NodeId, Visibility};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct LeakKey {
    from: NodeId,
    owner: NodeId,
    struct_name: String,
    field_name: String,
}

pub fn check(model: &Model, scope: Option<NodeId>) -> Vec<Diagnostic> {
    let tree = model.tree();
    let bounds = boundaries(tree);
    let field_counts = model.struct_field_counts();

    let mut grouped: BTreeMap<LeakKey, Vec<(String, u32)>> = BTreeMap::new();
    for reference in model.references().iter().filter(|r| !r.test) {
        let item = model.item(reference.item);
        if item.kind != ItemKind::Field
            || item.name.matches("::").count() >= 2
            || tree.is_test(item.owner)
            || tree.is_within(reference.from, item.owner)
        {
            continue;
        }
        let Some(struct_name) = item.parent_type.as_ref() else {
            continue;
        };
        let &(private, non_private) = field_counts
            .get(&(item.owner, struct_name.clone()))
            .unwrap_or(&(0, 0));
        let is_encapsulated =
            private >= 1 && (private >= non_private || item.visibility == Visibility::Crate);
        if !is_encapsulated {
            continue;
        }
        let from_boundary = home_boundary(tree, &bounds, reference.from);
        let owner_boundary = home_boundary(tree, &bounds, item.owner);
        if from_boundary == owner_boundary {
            continue;
        }
        if !in_scope(tree, scope, from_boundary) && !in_scope(tree, scope, owner_boundary) {
            continue;
        }
        let field_name = item
            .name
            .rsplit_once("::")
            .map_or(item.name.as_str(), |(_, f)| f)
            .to_owned();
        grouped
            .entry(LeakKey {
                from: from_boundary,
                owner: owner_boundary,
                struct_name: struct_name.clone(),
                field_name,
            })
            .or_default()
            .push((model.file(reference.file).path.clone(), reference.line));
    }

    grouped
        .into_iter()
        .map(|(key, mut sites)| {
            sites.sort();
            sites.dedup();
            let (file, line) = sites[0].clone();
            let from_str = tree.display(key.from);
            let owner_str = tree.display(key.owner);
            let mut notes = Vec::new();
            if sites.len() > 1 {
                let more: Vec<String> = sites[1..]
                    .iter()
                    .take(5)
                    .map(|(path, line)| clickable(path, *line))
                    .collect();
                let suffix = if sites.len() > 6 { ", ..." } else { "" };
                notes.push(format!(
                    "{} more in `{from_str}`: {}{suffix}",
                    sites.len() - 1,
                    more.join(", ")
                ));
            }
            Diagnostic {
                lint: Lint::FieldLeak,
                key: format!(
                    "{from_str} {owner_str}::{}::{}",
                    key.struct_name, key.field_name
                ),
                message: format!(
                    "`{from_str}` directly accesses field `{}::{}` of encapsulated struct in `{owner_str}`",
                    key.struct_name, key.field_name
                ),
                location: Some(Location { file, line }),
                notes,
                help: format!(
                    "keep `{}::{}` private and expose an operation on `{}` instead",
                    key.struct_name, key.field_name, key.struct_name
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{File, Item, NodeKind, Reference, Tree};

    #[test]
    fn flags_encapsulated_struct_and_skips_plain_data_and_public_dto() {
        let mut tree = Tree::new();
        let pkg_a = tree.child(tree.root(), NodeKind::Package, "a");
        let pkg_b = tree.child(tree.root(), NodeKind::Package, "b");
        let mut model = Model::new(tree);
        let file = model.add_file(File {
            path: "b/src/lib.rs".into(),
            lines: 10,
            module: pkg_b,
            test: false,
        });
        let add_field = |model: &mut Model, ty: &str, field: &str, vis| {
            model.add_item(Item {
                name: format!("{ty}::{field}"),
                owner: pkg_a,
                module: None,
                file: None,
                kind: ItemKind::Field,
                visibility: vis,
                signature: String::new(),
                parent_type: Some(ty.into()),
                is_trait_impl_item: false,
            })
        };
        // Encapsulated (1 private, 1 pub): flagged.
        add_field(&mut model, "Vault", "secret", Visibility::Private);
        let leaked = add_field(&mut model, "Vault", "state", Visibility::Public);
        // Plain data (0 private, 2 pub): allowed.
        let pod = add_field(&mut model, "Regs", "rax", Visibility::Public);
        add_field(&mut model, "Regs", "rbx", Visibility::Public);
        // Public DTO (1 private, 3 pub): allowed.
        add_field(&mut model, "Result", "raw", Visibility::Private);
        let dto = add_field(&mut model, "Result", "count", Visibility::Public);
        add_field(&mut model, "Result", "truncated", Visibility::Public);
        add_field(&mut model, "Result", "flags", Visibility::Public);

        for (line, item) in [(1, leaked), (2, pod), (3, dto)] {
            model.add_reference(Reference {
                file,
                line,
                from: pkg_b,
                from_function: None,
                item,
                test: false,
            });
        }
        let diags = check(&model, None);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].key, "b a::Vault::state");
    }
}
