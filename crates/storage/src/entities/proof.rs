//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.7

use super::sea_orm_active_enums::ProofType;
use super::sea_orm_active_enums::ScriptType;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "proof")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub txid: Vec<u8>,
    pub vout: i32,
    #[sea_orm(column_type = "VarBinary(StringLen::None)", nullable)]
    pub spend_txid: Option<Vec<u8>>,
    pub spend_vout: Option<i32>,
    pub is_frozen: bool,
    #[sea_orm(column_type = "VarBinary(StringLen::None)")]
    pub script: Vec<u8>,
    #[sea_orm(column_type = "VarBinary(StringLen::None)", nullable)]
    pub metadata: Option<Vec<u8>>,
    pub script_type: ScriptType,
    pub proof_type: ProofType,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::bulletproof::Entity")]
    Bulletproof,
    #[sea_orm(has_many = "super::inner_key::Entity")]
    InnerKey,
    #[sea_orm(
        belongs_to = "super::l1_transaction::Entity",
        from = "Column::Txid",
        to = "super::l1_transaction::Column::Txid",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    L1Transaction,
    #[sea_orm(has_many = "super::lightning_commitment_proof::Entity")]
    LightningCommitmentProof,
    #[sea_orm(has_many = "super::lightning_htlc_proof::Entity")]
    LightningHtlcProof,
    #[sea_orm(has_many = "super::multisig_proof::Entity")]
    MultisigProof,
    #[sea_orm(has_many = "super::p2wsh_proof::Entity")]
    P2wshProof,
    #[sea_orm(has_many = "super::spark_exit_proof::Entity")]
    SparkExitProof,
    #[sea_orm(has_many = "super::token::Entity")]
    Token,
}

impl Related<super::bulletproof::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bulletproof.def()
    }
}

impl Related<super::inner_key::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InnerKey.def()
    }
}

impl Related<super::l1_transaction::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::L1Transaction.def()
    }
}

impl Related<super::lightning_commitment_proof::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LightningCommitmentProof.def()
    }
}

impl Related<super::lightning_htlc_proof::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LightningHtlcProof.def()
    }
}

impl Related<super::multisig_proof::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MultisigProof.def()
    }
}

impl Related<super::p2wsh_proof::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::P2wshProof.def()
    }
}

impl Related<super::spark_exit_proof::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SparkExitProof.def()
    }
}

impl Related<super::token::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Token.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
