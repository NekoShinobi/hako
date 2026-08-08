use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "files")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub file_id: i64,
    pub user_id: Option<i64>,
    pub file_name: String,
    pub file_path: String,
    /// Unguessable public identifier — this is what appears in the URL.
    /// Never derived from the file's contents.
    pub token: String,
    pub created_at: DateTimeUtc,
    pub expires_at: Option<DateTimeUtc>,
    pub size: i64,
    pub mime_type: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::UserId"
    )]
    Users,
    #[sea_orm(has_many = "super::file_access_logs::Entity")]
    FileAccessLogs,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl Related<super::file_access_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FileAccessLogs.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
