use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
pub struct QuestAsset(pub lifthrasir_data::QuestData);
