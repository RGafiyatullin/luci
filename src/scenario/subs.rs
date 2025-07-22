use std::path::PathBuf;

use bimap::BiHashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    names::{ActorName, SubroutineName},
    scenario::{no_extra::NoExtra, DstPattern},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefDeclareSub {
    #[serde(rename = "load")]
    pub file_name: PathBuf,

    #[serde(rename = "as")]
    pub subroutine_name: SubroutineName,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefCallSub {
    #[serde(rename = "sub")]
    pub subroutine_name: SubroutineName,

    #[serde(rename = "in")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<DefSubBind>,

    #[serde(rename = "out")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<DefSubBind>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cast: Option<BiHashMap<ActorName, ActorName>>,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefSubBind {
    pub dst: DstPattern,
    pub src: Value,

    #[serde(flatten)]
    pub no_extra: NoExtra,
}
