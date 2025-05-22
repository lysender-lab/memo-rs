use std::result::Result as StdResult;

use crate::{Error, Result};
use memo::actor::Actor;
use memo::role::Permission;

pub enum Resource {
    Client,
    Bucket,
    User,
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
        Resource::Client => enforce_client_permissions(actor, action),
        Resource::Bucket => enforce_buckets_permissions(actor, action),
        Resource::User => enforce_users_permissions(actor, action),
        Resource::Album => enforce_dir_permissions(actor, action),
        Resource::Photo => enforce_photo_permissions(actor, action),
    };

    match result {
        Ok(_) => Ok(()),
        Err(message) => Err(Error::Forbidden {
            msg: message.to_string(),
        }),
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

fn enforce_client_permissions(actor: &Actor, action: Action) -> StdResult<(), &str> {
    let (permissions, message) = match action {
        Action::Create => (
            vec![Permission::ClientsCreate],
            "You do not have permission to create new clients.",
        ),
        Action::Read => (
            vec![Permission::ClientsList, Permission::ClientsView],
            "You do not have permission to view clients.",
        ),
        Action::Update => (
            vec![Permission::ClientsEdit],
            "You do not have permission to edit clients.",
        ),
        Action::Delete => (
            vec![Permission::ClientsDelete],
            "You do not have permission to delete clients.",
        ),
    };

    if !actor.has_permissions(&permissions) {
        return Err(message);
    }
    Ok(())
}

fn enforce_buckets_permissions(actor: &Actor, action: Action) -> StdResult<(), &str> {
    let (permissions, message) = match action {
        Action::Create => (
            vec![Permission::BucketsCreate],
            "You do not have permission to create new buckets.",
        ),
        Action::Read => (
            vec![Permission::BucketsList, Permission::BucketsView],
            "You do not have permission to view buckets.",
        ),
        Action::Update => (
            vec![Permission::BucketsEdit],
            "You do not have permission to edit buckets.",
        ),
        Action::Delete => (
            vec![Permission::BucketsDelete],
            "You do not have permission to delete buckets.",
        ),
    };

    if !actor.has_permissions(&permissions) {
        return Err(message);
    }
    Ok(())
}

fn enforce_users_permissions(actor: &Actor, action: Action) -> StdResult<(), &str> {
    let (permissions, message) = match action {
        Action::Create => (
            vec![Permission::UsersCreate],
            "You do not have permission to create new users.",
        ),
        Action::Read => (
            vec![Permission::UsersList, Permission::UsersView],
            "You do not have permission to view users.",
        ),
        Action::Update => (
            vec![Permission::UsersEdit],
            "You do not have permission to edit users.",
        ),
        Action::Delete => (
            vec![Permission::UsersDelete],
            "You do not have permission to delete users.",
        ),
    };

    if !actor.has_permissions(&permissions) {
        return Err(message);
    }
    Ok(())
}
