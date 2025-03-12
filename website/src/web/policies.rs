use crate::models::{Actor, Permission};
use std::result::Result as StdResult;

use crate::{Error, Result};

pub enum Resource {
    Album,
    Photo,
}

pub enum Action {
    Create,
    Read,
    Update,
    Delete,
}

pub fn enforce_policy(actor: &Actor, resource: Resource, action: Action) -> Result<()> {
    let result = match resource {
        Resource::Album => enforce_dir_permissions(actor, action),
        Resource::Photo => enforce_photo_permissions(actor, action),
    };

    match result {
        Ok(_) => Ok(()),
        Err(message) => Err(Error::Forbidden(message.to_string())),
    }
}

fn enforce_dir_permissions(actor: &Actor, action: Action) -> StdResult<(), &str> {
    let (permissions, message) = match action {
        Action::Create => (
            vec![Permission::DirsCreate],
            "You do not have permission to create albums.",
        ),
        Action::Read => (
            vec![Permission::DirsList, Permission::DirsView],
            "You do not have permission to view albums.",
        ),
        Action::Update => (
            vec![Permission::DirsEdit],
            "You do not have permission to edit albums.",
        ),
        Action::Delete => (
            vec![Permission::DirsDelete],
            "You do not have permission to delete albums.",
        ),
    };

    if !actor.has_permissions(&permissions) {
        return Err(message);
    }
    Ok(())
}

fn enforce_photo_permissions(actor: &Actor, action: Action) -> StdResult<(), &str> {
    let (permissions, message) = match action {
        Action::Create => (
            vec![Permission::FilesCreate],
            "You do not have permission to upload photos.",
        ),
        Action::Read => (
            vec![Permission::FilesList, Permission::FilesView],
            "You do not have permission to view photos.",
        ),
        Action::Update => (
            vec![Permission::FilesEdit],
            "You do not have permission to edit photos.",
        ),
        Action::Delete => (
            vec![Permission::FilesDelete],
            "You do not have permission to delete photos.",
        ),
    };

    if !actor.has_permissions(&permissions) {
        return Err(message);
    }
    Ok(())
}
