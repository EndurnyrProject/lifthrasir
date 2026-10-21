//! Pure effect recipe composition and descriptor validation.

use super::{EffectData, EffectDescriptor, ShaderFxEntry, Visual};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

/// Invalid effect catalog, with the original section/id diagnostic.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("{0}")]
pub struct EffectCatalogError(String);

/// Resolve recipes and validate all descriptor namespaces without asset loading.
pub fn resolve_effect_catalog(
    data: &EffectData,
) -> Result<ResolvedEffectCatalog, EffectCatalogError> {
    ResolvedEffectCatalog::build(data).map_err(EffectCatalogError)
}

/// The descriptor tables of `effects.ron`, resolved and validated once at
/// load: `skills` (rAthena skill id), `skill_units` (rAthena skill id, the
/// persistent visual of a skill whose `skills` entry is a one-shot cast),
/// `special` (the `e_special_effects` EF_* id, driving both RSW map effects and
/// aesir `SpecialEffect` packets), and `efsts` (EFST id). The id namespaces are
/// distinct, so the tables stay separate.
///
/// `Visual::Efst` layers in `skills`/`skill_units`/`special` are spliced with the referenced
/// `efsts` recipe's layers here, so lookups return descriptors whose `visuals`
/// carry no composition left to resolve.
#[derive(Debug)]
pub struct ResolvedEffectCatalog {
    skills: HashMap<u32, EffectDescriptor>,
    skill_units: HashMap<u32, EffectDescriptor>,
    special: BTreeMap<u32, EffectDescriptor>,
    efsts: HashMap<u32, EffectDescriptor>,
}

impl ResolvedEffectCatalog {
    /// Splice `Efst` composition and validate every table. `Err` carries a
    /// message naming the offending section and id, ready for `error!`.
    fn build(data: &EffectData) -> Result<Self, String> {
        for (id, descriptor) in &data.efsts {
            if let Some(nested) = first_efst_ref(descriptor) {
                return Err(format!(
                    "efsts[{id}] contains Efst({nested}); efst recipes are one level deep and must not compose other recipes"
                ));
            }
        }

        let skills = resolve_section("skills", &data.skills, &data.efsts)?;
        let skill_units = resolve_section("skill_units", &data.skill_units, &data.efsts)?;
        let special = resolve_section("special", &data.special, &data.efsts)?;

        if let Some(id) = skill_units
            .iter()
            .find_map(|(id, descriptor)| (!descriptor.repeating).then_some(id))
        {
            return Err(format!(
                "skill_units[{id}] must be repeating: true; a skill unit's visual is persistent"
            ));
        }

        for (section, table) in [
            ("skills", &skills),
            ("skill_units", &skill_units),
            ("special", &special),
            ("efsts", &data.efsts),
        ] {
            for (id, descriptor) in table {
                validate_descriptor(section, *id, descriptor, &data.shader_fx)?;
            }
        }

        Ok(Self {
            skills: skills.into_iter().collect(),
            skill_units: skill_units.into_iter().collect(),
            special,
            efsts: data.efsts.clone().into_iter().collect(),
        })
    }

    pub fn skill(&self, skill_id: u32) -> Option<&EffectDescriptor> {
        self.skills.get(&skill_id)
    }

    /// The persistent visual of a skill-unit group: its `skill_units` entry,
    /// else a repeating `skills` entry. A non-repeating `skills` entry is a
    /// one-shot whose visual belongs to the cast or damage trigger, not the
    /// group (some ground skills place a short-lived scheduling group).
    pub fn skill_unit(&self, skill_id: u32) -> Option<&EffectDescriptor> {
        self.skill_units
            .get(&skill_id)
            .or_else(|| self.skills.get(&skill_id).filter(|d| d.repeating))
    }

    pub fn special(&self, effect_id: u32) -> Option<&EffectDescriptor> {
        self.special.get(&effect_id)
    }

    /// Every `special` id, ascending.
    pub fn special_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.special.keys().copied()
    }

    pub fn efst(&self, efst_id: u32) -> Option<&EffectDescriptor> {
        self.efsts.get(&efst_id)
    }
}

/// The first `Efst` layer's target id, if the descriptor composes one.
fn first_efst_ref(descriptor: &EffectDescriptor) -> Option<u32> {
    descriptor.visuals.iter().find_map(|visual| match visual {
        Visual::Efst(id) => Some(*id),
        _ => None,
    })
}

/// Replace every `Efst(id)` layer with the referenced recipe's layers, in
/// place. Only `visuals` is touched: the composing entry keeps its own
/// placement, repeating, color and sound.
fn resolve_section(
    section: &str,
    table: &BTreeMap<u32, EffectDescriptor>,
    efsts: &BTreeMap<u32, EffectDescriptor>,
) -> Result<BTreeMap<u32, EffectDescriptor>, String> {
    table
        .iter()
        .map(|(id, descriptor)| {
            let mut resolved = descriptor.clone();
            resolved.visuals = Vec::new();
            for visual in &descriptor.visuals {
                let Visual::Efst(efst_id) = visual else {
                    resolved.visuals.push(visual.clone());
                    continue;
                };
                let recipe = efsts.get(efst_id).ok_or_else(|| {
                    format!("{section}[{id}] composes Efst({efst_id}), which has no efsts entry")
                })?;
                resolved.visuals.extend(recipe.visuals.iter().cloned());
            }
            Ok((*id, resolved))
        })
        .collect()
}

fn kind_name(visual: &Visual) -> &'static str {
    match visual {
        Visual::Str(_) => "Str",
        Visual::Sprite(_) => "Sprite",
        Visual::Model(_) => "Model",
        Visual::Shader(_) => "Shader",
        Visual::Bespoke(_) => "Bespoke",
        Visual::Efst(_) => "Efst",
    }
}

/// Every `Shader` layer must name a real `shader_fx` entry, and a resolved
/// descriptor may carry at most one layer of each kind.
///
/// NOTE: the one-layer-per-kind ceiling is deliberate — the dispatch code reads
/// the first layer of a kind, so a second would render as a silent drop. Lift it
/// (accessors returning iterators, dispatch looping) when a real effect needs
/// two layers of one kind.
fn validate_descriptor(
    section: &str,
    id: u32,
    descriptor: &EffectDescriptor,
    shader_fx: &BTreeMap<String, ShaderFxEntry>,
) -> Result<(), String> {
    // `Str` only, deliberately narrower than "any one-shot kind": the trigger
    // spawner resolves its asset through `load_effect`, which handles `Str`
    // alone. `Sprite` is served by `EffectSprite`, a *persistent* marker with no
    // one-shot lifetime, so admitting it here would accept data that renders
    // nothing. Widen this only together with the spawner.
    if let Some(trigger) = &descriptor.on_trigger
        && !matches!(&trigger.visual, Visual::Str(_))
    {
        return Err(format!(
            "{section}[{id}] carries on_trigger {} visual; only Str is supported",
            kind_name(&trigger.visual)
        ));
    }

    // Rendering is gated on `repeating` alone -- that is what routes a descriptor
    // to the skill-unit group path that attaches a persistent visual. The
    // `placement: Ground` half is an authoring-intent guard, not a rendering
    // precondition: a model that is not a ground skill unit's prop is a mistake
    // worth failing on, even though it would technically draw.
    if descriptor
        .visuals
        .iter()
        .any(|visual| matches!(visual, Visual::Model(_)))
        && (!descriptor.repeating || descriptor.placement != super::EffectPlacement::Ground)
    {
        return Err(format!(
            "{section}[{id}] carries a Model layer, which requires repeating: true and placement: Ground"
        ));
    }

    let mut seen: Vec<&'static str> = Vec::with_capacity(descriptor.visuals.len());
    for visual in &descriptor.visuals {
        if let Visual::Shader(key) = visual
            && !shader_fx.contains_key(key)
        {
            return Err(format!(
                "{section}[{id}] references Shader(\"{key}\"), which has no shader_fx entry"
            ));
        }

        let kind = kind_name(visual);
        if seen.contains(&kind) {
            return Err(format!(
                "{section}[{id}] carries more than one {kind} layer; at most one layer per kind is supported"
            ));
        }
        seen.push(kind);
    }
    Ok(())
}
