use std::path::Path;

use snafu::ResultExt;
use turso::{Builder, Connection};

use crate::db::{
    app::AppRepo, oauth_code::OauthCodeRepo, org::OrgRepo, org_app::OrgAppRepo,
    org_member::OrgMemberRepo, password::PasswordRepo, superuser::SuperuserRepo, user::UserRepo,
};
use crate::error::{DbBuilderSnafu, DbConnectSnafu};

use crate::Result;

pub async fn create_db_pool(filename: &Path) -> Result<Connection> {
    let db = Builder::new_local(filename.to_str().expect("DB path is required"))
        .build()
        .await
        .context(DbBuilderSnafu)?;
    let conn = db.connect().context(DbConnectSnafu)?;

    // Enable MVCC
    conn.pragma_update("journal_mode", "'mvcc'")
        .await
        .context(DbConnectSnafu)?;

    Ok(conn)
}

pub struct DbMapper {
    pub apps: AppRepo,
    pub oauth_codes: OauthCodeRepo,
    pub orgs: OrgRepo,
    pub org_apps: OrgAppRepo,
    pub org_members: OrgMemberRepo,
    pub passwords: PasswordRepo,
    pub superusers: SuperuserRepo,
    pub users: UserRepo,
}

pub async fn create_db_mapper(filename: &Path) -> Result<DbMapper> {
    let pool = create_db_pool(filename).await?;
    Ok(DbMapper {
        apps: AppRepo::new(pool.clone()),
        oauth_codes: OauthCodeRepo::new(pool.clone()),
        orgs: OrgRepo::new(pool.clone()),
        org_apps: OrgAppRepo::new(pool.clone()),
        org_members: OrgMemberRepo::new(pool.clone()),
        passwords: PasswordRepo::new(pool.clone()),
        superusers: SuperuserRepo::new(pool.clone()),
        users: UserRepo::new(pool),
    })
}
