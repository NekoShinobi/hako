use sea_orm_migration::prelude::*;

/// Initial schema.
///
/// Two design choices are load-bearing and worth stating here, since the column
/// names alone don't explain them:
///
/// - `files.token` is an unguessable public identifier, never derived from the
///   file's contents. An earlier design used the SHA-256 of the file itself,
///   which meant anyone holding a copy could derive its link, and a duplicate
///   upload tripped the UNIQUE constraint into a 500 that confirmed the file
///   existed.
/// - `users.ip_hash` holds a keyed HMAC-SHA256 of the uploader's address, never
///   the address itself.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create users table
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Users::UserId)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Users::IpHash).string_len(64).not_null())
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Users::SessionToken)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Users::IsAnonymous)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .to_owned(),
            )
            .await?;

        // Create files table
        manager
            .create_table(
                Table::create()
                    .table(Files::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Files::FileId)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Files::UserId).big_integer())
                    .col(ColumnDef::new(Files::FileName).string().not_null())
                    .col(ColumnDef::new(Files::FilePath).string().not_null())
                    .col(
                        ColumnDef::new(Files::Token)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Files::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Files::ExpiresAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Files::Size).big_integer().not_null())
                    .col(ColumnDef::new(Files::MimeType).string())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_files_user_id")
                            .from(Files::Table, Files::UserId)
                            .to(Users::Table, Users::UserId)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Create file_access_logs table
        manager
            .create_table(
                Table::create()
                    .table(FileAccessLogs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FileAccessLogs::LogId)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FileAccessLogs::FileId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FileAccessLogs::AccessedBy)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FileAccessLogs::AccessTime)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_file_access_logs_file_id")
                            .from(FileAccessLogs::Table, FileAccessLogs::FileId)
                            .to(Files::Table, Files::FileId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_files_token")
                    .table(Files::Table)
                    .col(Files::Token)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_users_session")
                    .table(Users::Table)
                    .col(Users::SessionToken)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_file_access_file_id")
                    .table(FileAccessLogs::Table)
                    .col(FileAccessLogs::FileId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FileAccessLogs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Files::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    UserId,
    IpHash,
    CreatedAt,
    SessionToken,
    IsAnonymous,
}

#[derive(DeriveIden)]
enum Files {
    Table,
    FileId,
    UserId,
    FileName,
    FilePath,
    Token,
    CreatedAt,
    ExpiresAt,
    Size,
    MimeType,
}

#[derive(DeriveIden)]
enum FileAccessLogs {
    Table,
    LogId,
    FileId,
    AccessedBy,
    AccessTime,
}
