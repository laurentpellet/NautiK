//! Application des fichiers de mise à jour S-57 (`.001`, `.002`…), selon
//! IHO S-57 édition 3.1, partie 3, § 8.4.
//!
//! Chaque enregistrement d'une mise à jour porte une instruction (RUIN) :
//! insérer, supprimer ou modifier l'enregistrement de même identifiant.
//! Une suppression ou une modification n'est acceptée que si la version
//! annoncée (RVER) suit celle de l'enregistrement visé. Les listes
//! (pointeurs, coordonnées) sont modifiées par les champs de contrôle
//! VRPC, SGCC et FSPC ; un attribut de valeur `DEL` (0x7F) est supprimé.
//!
//! Une mise à jour est appliquée sur une copie de la cellule : à la
//! moindre incohérence, elle est refusée en entier, sans état
//! intermédiaire.

use super::geometry::RawVector;
use super::{
    read_dataset, update_number, CellState, Dataset, FeatureState, ListUpdate, S57Attribute,
    EXPP_UPDATE, RUIN_INSERT,
};

/// Instruction « supprimer » (RUIN).
const RUIN_DELETE: u8 = 2;
/// Instruction « modifier » (RUIN).
const RUIN_MODIFY: u8 = 3;

/// Instructions de mise à jour de liste.
const LIST_INSERT: u8 = 1;
const LIST_DELETE: u8 = 2;
const LIST_MODIFY: u8 = 3;

/// Édition annonçant l'annulation de la cellule (S-57 3.1, appendice B.1).
const CANCELLED_EDITION: &str = "0";

/// Valeur d'attribut signifiant « supprimer l'attribut ».
const DELETE_VALUE: &str = "\u{7f}";

/// Résultat de l'application d'un fichier de mise à jour.
#[derive(Debug)]
pub(crate) enum Applied {
    /// Mise à jour déjà intégrée à la cellule de base (réédition).
    AlreadyIncluded,
    /// Mise à jour appliquée : nouvel état et date de publication.
    Updated {
        state: Box<CellState>,
        issue_date: String,
    },
    /// Cellule annulée par le producteur (édition 0) : date d'annulation.
    Cancelled { issue_date: String },
}

/// Applique un fichier de mise à jour à une copie de la cellule.
pub(crate) fn apply_file(state: &CellState, bytes: &[u8]) -> Result<Applied, String> {
    let dataset = read_dataset(bytes).map_err(|e| e.to_string())?;
    let id = &dataset.id;
    if id.expp != EXPP_UPDATE {
        return Err(format!(
            "{} n'est pas un fichier de mise à jour (EXPP {})",
            id.name, id.expp
        ));
    }
    let cancelled = id.edition.trim() == CANCELLED_EDITION;
    if !cancelled && id.edition.trim() != state.id.edition.trim() {
        return Err(format!(
            "édition {} au lieu de {}",
            id.edition, state.id.edition
        ));
    }
    let number = update_number(&id.update).map_err(|e| e.to_string())?;
    if number <= state.update_number {
        return Ok(Applied::AlreadyIncluded);
    }
    let expected = state.update_number + 1;
    if number != expected {
        return Err(format!(
            "mise à jour n° {expected} manquante (fichier n° {number})"
        ));
    }
    let issue_date = id.issue_date.clone();
    if cancelled {
        return Ok(Applied::Cancelled { issue_date });
    }
    let mut next = state.clone();
    apply_records(&mut next, dataset)?;
    next.update_number = number;
    Ok(Applied::Updated {
        state: Box::new(next),
        issue_date,
    })
}

/// Vérifie qu'une version suit la version actuelle.
fn check_version(what: &str, current: u16, announced: u16) -> Result<(), String> {
    if current.checked_add(1) == Some(announced) {
        Ok(())
    } else {
        Err(format!(
            "{what} : version {announced} inattendue (version actuelle {current})"
        ))
    }
}

/// Modifie une liste selon une instruction VRPC, SGCC ou FSPC.
pub(crate) fn edit_list<T>(
    list: &mut Vec<T>,
    control: ListUpdate,
    items: Vec<T>,
    what: &str,
) -> Result<(), String> {
    let bad = || {
        format!(
            "{what} : instruction {} à la position {} pour {} élément(s) incohérente (liste de {})",
            control.instruction,
            control.index,
            control.count,
            list.len()
        )
    };
    let start = control.index.checked_sub(1).ok_or_else(bad)?;
    let end = start.checked_add(control.count).ok_or_else(bad)?;
    match control.instruction {
        LIST_INSERT if items.len() == control.count && start <= list.len() => {
            list.splice(start..start, items);
        }
        LIST_DELETE if end <= list.len() => {
            list.drain(start..end);
        }
        LIST_MODIFY if items.len() == control.count && end <= list.len() => {
            list.splice(start..end, items);
        }
        _ => return Err(bad()),
    }
    Ok(())
}

/// Applique les modifications d'attributs : remplacement, ajout, ou
/// suppression (valeur `DEL`).
fn edit_attributes(attributes: &mut Vec<S57Attribute>, changes: Vec<S57Attribute>) {
    for change in changes {
        let position = attributes.iter().position(|a| a.code == change.code);
        match (position, change.value == DELETE_VALUE) {
            (Some(i), true) => {
                attributes.remove(i);
            }
            (None, true) => {}
            (Some(i), false) => attributes[i] = change,
            (None, false) => attributes.push(change),
        }
    }
}

fn apply_records(state: &mut CellState, dataset: Dataset) -> Result<(), String> {
    for record in dataset.vectors {
        let key = (record.rcnm, record.rcid);
        let what = format!("vecteur {}/{}", record.rcnm, record.rcid);
        match record.instruction {
            RUIN_INSERT => {
                if state.vectors.contains_key(&key) {
                    return Err(format!("{what} : déjà présent"));
                }
                state.vectors.insert(key, record.vector);
                state.vector_versions.insert(key, record.version);
            }
            RUIN_DELETE | RUIN_MODIFY => {
                let current = *state
                    .vector_versions
                    .get(&key)
                    .ok_or_else(|| format!("{what} : introuvable"))?;
                check_version(&what, current, record.version)?;
                if record.instruction == RUIN_DELETE {
                    state.vectors.remove(&key);
                    state.vector_versions.remove(&key);
                    continue;
                }
                let target = state
                    .vectors
                    .get_mut(&key)
                    .ok_or_else(|| format!("{what} : introuvable"))?;
                modify_vector(
                    target,
                    record.vector,
                    record.pointer_update,
                    record.coord_update,
                    &what,
                )?;
                state.vector_versions.insert(key, record.version);
            }
            other => return Err(format!("{what} : instruction {other} inconnue")),
        }
    }
    for record in dataset.features {
        let what = record.label();
        match record.instruction {
            RUIN_INSERT => {
                if state.features.contains_key(&record.rcid) {
                    return Err(format!("{what} : déjà présent"));
                }
                state.features.insert(
                    record.rcid,
                    FeatureState {
                        version: record.version,
                        prim: record.prim,
                        group: record.group,
                        objl: record.objl,
                        attributes: record.attributes,
                        pointers: record.pointers,
                    },
                );
            }
            RUIN_DELETE => {
                let current = state
                    .features
                    .get(&record.rcid)
                    .ok_or_else(|| format!("{what} : introuvable"))?;
                check_version(&what, current.version, record.version)?;
                state.features.remove(&record.rcid);
            }
            RUIN_MODIFY => {
                let target = state
                    .features
                    .get_mut(&record.rcid)
                    .ok_or_else(|| format!("{what} : introuvable"))?;
                check_version(&what, target.version, record.version)?;
                edit_attributes(&mut target.attributes, record.attributes);
                match record.pointer_update {
                    Some(control) => {
                        edit_list(&mut target.pointers, control, record.pointers, &what)?;
                    }
                    None if !record.pointers.is_empty() => {
                        return Err(format!("{what} : pointeurs sans instruction FSPC"));
                    }
                    None => {}
                }
                target.version = record.version;
            }
            other => return Err(format!("{what} : instruction {other} inconnue")),
        }
    }
    Ok(())
}

/// Modifie les pointeurs (VRPC) et les coordonnées (SGCC) d'un vecteur.
fn modify_vector(
    target: &mut RawVector,
    changes: RawVector,
    pointer_update: Option<ListUpdate>,
    coord_update: Option<ListUpdate>,
    what: &str,
) -> Result<(), String> {
    match pointer_update {
        Some(control) => edit_list(&mut target.pointers, control, changes.pointers, what)?,
        None if !changes.pointers.is_empty() => {
            return Err(format!("{what} : pointeurs sans instruction VRPC"));
        }
        None => {}
    }
    match coord_update {
        Some(control) => {
            let target_3d = !target.depths.is_empty();
            let changes_3d = !changes.depths.is_empty();
            if !changes.coords.is_empty() && target_3d != changes_3d && !target.coords.is_empty() {
                return Err(format!("{what} : coordonnées 2D et 3D mélangées"));
            }
            if target_3d || changes_3d {
                edit_list(&mut target.depths, control, changes.depths, what)?;
            }
            edit_list(&mut target.coords, control, changes.coords, what)?;
        }
        None if !changes.coords.is_empty() => {
            return Err(format!("{what} : coordonnées sans instruction SGCC"));
        }
        None => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(instruction: u8, index: usize, count: usize) -> ListUpdate {
        ListUpdate {
            instruction,
            index,
            count,
        }
    }

    #[test]
    fn edits_lists_by_position() {
        let mut list = vec![1, 2, 3];
        edit_list(&mut list, control(LIST_INSERT, 2, 2), vec![8, 9], "t").unwrap();
        assert_eq!(list, [1, 8, 9, 2, 3]);
        edit_list(&mut list, control(LIST_INSERT, 6, 1), vec![7], "t").unwrap();
        assert_eq!(list, [1, 8, 9, 2, 3, 7]);
        edit_list(&mut list, control(LIST_DELETE, 1, 3), Vec::new(), "t").unwrap();
        assert_eq!(list, [2, 3, 7]);
        edit_list(&mut list, control(LIST_MODIFY, 3, 1), vec![5], "t").unwrap();
        assert_eq!(list, [2, 3, 5]);
    }

    #[test]
    fn rejects_inconsistent_list_edits() {
        let mut list = vec![1, 2, 3];
        for (c, items) in [
            (control(LIST_INSERT, 0, 1), vec![4]),
            (control(LIST_INSERT, 5, 1), vec![4]),
            (control(LIST_INSERT, 1, 2), vec![4]),
            (control(LIST_DELETE, 3, 2), Vec::new()),
            (control(LIST_MODIFY, 3, 2), vec![4, 5]),
            (control(LIST_MODIFY, 1, 1), Vec::new()),
            (control(9, 1, 1), vec![4]),
            (control(LIST_DELETE, usize::MAX, 2), Vec::new()),
        ] {
            assert!(edit_list(&mut list, c, items, "t").is_err(), "{c:?}");
        }
        assert_eq!(list, [1, 2, 3]);
    }

    #[test]
    fn edits_attributes() {
        let attr = |code, value: &str| S57Attribute {
            code,
            value: value.into(),
        };
        let mut attributes = vec![attr(116, "Ancien"), attr(179, "3.5")];
        edit_attributes(
            &mut attributes,
            vec![
                attr(116, "Nouveau"),
                attr(179, DELETE_VALUE),
                attr(75, "4"),
                attr(1, DELETE_VALUE),
            ],
        );
        assert_eq!(attributes, [attr(116, "Nouveau"), attr(75, "4")]);
    }

    #[test]
    fn versions_must_follow() {
        assert!(check_version("x", 1, 2).is_ok());
        assert!(check_version("x", 1, 1).is_err());
        assert!(check_version("x", 1, 3).is_err());
        assert!(check_version("x", u16::MAX, 0).is_err());
    }
}
