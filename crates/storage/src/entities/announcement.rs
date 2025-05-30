//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.7

use super::sea_orm_active_enums::AnnouncementType;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "announcement")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "VarBinary(StringLen::None)", unique)]
    pub txid: Vec<u8>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub token_pubkey: Vec<u8>,
    pub r#type: AnnouncementType,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::issue_announcement::Entity")]
    IssueAnnouncement,
    #[sea_orm(
        belongs_to = "super::l1_transaction::Entity",
        from = "Column::Txid",
        to = "super::l1_transaction::Column::Txid",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    L1Transaction,
    #[sea_orm(has_one = "super::pubkey_freeze_announcement::Entity")]
    PubkeyFreezeAnnouncement,
    #[sea_orm(has_one = "super::token_logo_announcement::Entity")]
    TokenLogoAnnouncement,
    #[sea_orm(has_one = "super::token_pubkey_announcement::Entity")]
    TokenPubkeyAnnouncement,
    #[sea_orm(has_one = "super::transfer_ownership_announcement::Entity")]
    TransferOwnershipAnnouncement,
    #[sea_orm(has_one = "super::tx_freeze_announcement::Entity")]
    TxFreezeAnnouncement,
}

impl Related<super::issue_announcement::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::IssueAnnouncement.def()
    }
}

impl Related<super::l1_transaction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::L1Transaction.def()
    }
}

impl Related<super::pubkey_freeze_announcement::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PubkeyFreezeAnnouncement.def()
    }
}

impl Related<super::token_logo_announcement::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TokenLogoAnnouncement.def()
    }
}

impl Related<super::token_pubkey_announcement::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TokenPubkeyAnnouncement.def()
    }
}

impl Related<super::transfer_ownership_announcement::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TransferOwnershipAnnouncement.def()
    }
}

impl Related<super::tx_freeze_announcement::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TxFreezeAnnouncement.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
