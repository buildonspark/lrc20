//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.7

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "p2wsh_proof")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub txid: Vec<u8>,
    pub vout: i32,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub script: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::proof::Entity",
        from = "(Column::Txid, Column::Vout)",
        to = "(super::proof::Column::Txid, super::proof::Column::Vout)",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Proof,
}

impl Related<super::proof::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Proof.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
