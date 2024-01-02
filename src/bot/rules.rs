use super::{repl, send, Context, Result};
use crate::bot::get_nextblock;
use emoji::named::*;
use futures_util::StreamExt;
use poise::serenity_prelude::*;
use tokio::sync::Mutex;
use tokio::sync::{broadcast, OnceCell};

macro_rules! val {
    ($($k:ident($v:ty)),+) => {
        #[derive(Clone, Debug)]
        pub enum Val {
            $($k($v)),+
        }

        $(
            impl From<$v> for Val {
                fn from(value: $v) -> Self {
                    Self::$k(value)
                }
            }
        )+

        impl std::fmt::Display for Val {
            fn fmt(&self, f:&mut  std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$k(x) => write!(f, "{x}"),)+
                }
            }
        }
    }
}

val! {
    Bool(bool),
    Float(f32),
    Int(i32),
    Str(String)
}

macro_rules! rules {
    { of $s: ident :
        $($k:literal: $ty:ty [$doc:literal]),+ $(,)?
    } => { paste::paste! {
        impl $s {
            pub fn reduce(&self) -> impl Iterator<Item = (&'static str, Val)> {
                [$(self. [ < $k : snake > ].clone().map(Val::from).map(|x| ($k, x))),+].into_iter().filter_map(|x| x.clone())
            }

            pub fn name() -> &'static [&'static str] {
                &[$(stringify!([<$k:snake>])),+]
            }

            pub fn set(&mut self, x: &str,to: &str) -> Result<()> {
                match x {
                    $(stringify!([< $k : snake> ]) => {
                        self.[ < $k : snake >] = Some(to.parse()?);
                    }),+
                    _ => anyhow::bail!("no such field"),
                }
                Ok(())
            }

            /// returns [`None`] if field not found,
            /// returns [`Some`](if the field was not none)
            pub fn delete(&mut self, x: &str) -> Option<bool> {
                match x {
                    $(stringify!([< $k : snake> ]) => {
                        match &mut self.[ < $k : snake >] {
                            y @ Some(_) => {
                                *y = None;
                                Some(true)
                            },
                            _ => {
                                Some(false)
                            }
                        }
                    }),+
                    _ => None,
                }
            }
        }


        #[derive(serde_derive::Serialize, serde_derive::Deserialize)]
        pub struct $s {
            $(
                #[serde(default)]
                #[serde(rename = $k)]
                #[serde(skip_serializing_if = "Option::is_none")]
                pub [< $k : snake>]: Option<$ty>,
            )+
        }
    } }
}

rules!(
    of Rules:
    "infiniteResources": bool ["Sandbox mode: Enables infinite resources, build range and build speed."],
    // mindustry does a wackky thing here, but
    // "teams": HashMap<u8, TeamRule> ["Team-specific rules."],
    "coreCapture": bool ["Whether cores change teams when they are destroyed."],
    "reactorExplosions": bool ["Whether reactors can explode and damage other blocks."],
    "possessionAllowed": bool ["Whether to allow manual unit control."],
    "schematicsAllowed": bool ["Whether schematics are allowed."],
    "damageExplosions": bool ["Whether friendly explosions can occur and set fire/damage other blocks."],
    "fire": bool ["Whether fire (and neoplasm spread) is enabled."],
    "unitAmmo": bool ["Whether units use and require ammo."],
    "unitPayloadUpdate": bool ["EXPERIMENTAL! If true, blocks will update in units and share power."],
    "unitCapVariable": bool ["Whether cores add to unit limit"],
    "showSpawns": bool ["If true, unit spawn points are shown."],
    "solarMultiplier": f32 ["Multiplies power output of solar panels."],
    "unitBuildSpeedMultiplier": f32 ["How fast unit factories build units."],
    "unitCostMultiplier": f32 ["Multiplier of resources that units take to build."],
    "unitDamageMultiplier": f32 ["How much damage units deal."],
    "unitHealthMultiplier": f32 ["How much health units start with."],
    "unitCrashDamageMultiplier": f32 ["How much damage unit crash damage deals. (Compounds with unitDamageMultiplier)"],
    "ghostBlocks": bool ["If true, ghost blocks will appear upon destruction, letting builder blocks/units rebuild them."],
    "logicUnitBuild": bool ["Whether to allow units to build with logic."],
    "disableWorldProcessors": bool ["If true, world processors no longer update. Used for testing."],
    "blockHealthMultiplier": f32 ["How much health blocks start with."],
    "blockDamageMultiplier": f32 ["How much damage blocks (turrets) deal."],
    "buildCostMultiplier": f32 ["Multiplier for buildings resource cost."],
    "buildSpeedMultiplier": f32 ["Multiplier for building speed."],
    "deconstructRefundMultiplier": f32 ["Multiplier for percentage of materials refunded when deconstructing."],
    "enemyCoreBuildRadius": f32 ["No-build zone around enemy core radius."],
    "polygonCoreProtection": bool ["If true, no-build zones are calculated based on the closest core."],
    "placeRangeCheck": bool ["If true, blocks cannot be placed near blocks that are near the enemy team."],
    "cleanupDeadTeams": bool ["If true, dead teams in PvP automatically have their blocks & units converted to derelict upon death."],
    "onlyDepositCore": bool ["If true, items can only be deposited in the core."],
    "coreDestroyClear": bool ["If true, every enemy block in the radius of the (enemy) core is destroyed upon death. Used for campaign maps."],
    "hideBannedBlocks": bool ["If true, banned blocks are hidden from the build menu."],
    "blockWhitelist": bool ["If true, bannedBlocks becomes a whitelist."],
    "unitWhitelist": bool ["If true, bannedUnits becomes a whitelist."],
    "unitCap": i32 ["Base unit cap. Can still be increased by blocks."],
    "dragMultiplier": f32 ["Environment drag multiplier."],
    "env": i32["Environmental flags that dictate visuals & how blocks function."],
    // TODO
    // "weather": Vec<Weather> ["Weather events that occur here."],
    // "bannedBlocks": Vec<Block> ["Blocks that cannot be placed."],
    // "bannedUnits": Vec<Unit> ["Units that cannot be built."],
    // "revealedBlocks": Vec<Block> ["Reveals blocks normally hidden by build visibility."],
    // "hiddenBuildItems": Vec<Item> ["Block containing these items as requirements are hidden."],
    // "objectives": MapObjectives ["In-map objective executor."],
    // "objectiveFlags": ObjectSet<String> ["Flags set by objectives. Used in world processors."],
    // "planet": Planet ["Rules from this planet are applied. If it's "sun", mixed tech is enabled."],
    "fog": bool ["If true, fog of war is enabled. Enemy units and buildings are hidden unless in radar view."],
    "staticFog": bool ["If fog = true, this is whether static (black) fog is enabled."],
    "staticColor": String ["Color for static, undiscovered fog of war areas."],
    "dynamicColor": String ["Color for discovered but un-monitored fog of war areas."],
    "lighting": bool ["Whether ambient lighting is enabled."],
    "ambientLight": String ["Ambient light color, used when lighting is enabled."],
    "modeName": String ["name of the custom mode that this ruleset describes, or null."],
    "mission": String ["Mission string displayed instead of wave/core counter. Null to disable."],
    "coreIncinerates": bool ["Whether cores incinerate items when full, just like in the campaign."],
    "borderDarkness": bool ["If false, borders fade out into darkness. Only use with custom backgrounds!"],
    "backgroundTexture": String ["path to background texture with extension (e.g. \"sprites/space.png\")"],
    "backgroundSpeed": f32 ["background texture move speed scaling - bigger numbers mean slower movement. 0 to disable."],
    "backgroundScl": f32 ["background texture scaling factor"],
    "backgroundOffsetX": f32 ["background UV offsets"],

);

// rules!(
//     of TeamRule:
//     "aiCoreSpawn": bool ["Whether, when AI is enabled, ships should be spawned from the core."],
//     "cheat": bool ["If true, blocks don't require power or resources."],
//     "infiniteResources": bool ["If true, resources are not consumed when building."],
//     "infiniteAmmo": bool ["If true, this team has infinite unit ammo."],
//     "buildAi": bool ["AI that builds random schematics."],
//     "buildAiTier": f32 ["Tier of builder AI. [0, 1]"],
//     "rtsAi": bool ["Enables \"RTS\" unit AI."],
//     "rtsMinSquad": i32["Minimum size of attack squads."],
//     "rtsMaxSquad": i32["Maximum size of attack squads."],
//     "rtsMinWeight": f32 ["Minimum \"advantage\" needed for a squad to attack. Higher -> more cautious."],
//     "unitBuildSpeedMultiplier": f32 ["How fast unit factories build units."],
//     "unitDamageMultiplier": f32 ["How much damage units deal."],
//     "unitCrashDamageMultiplier": f32 ["How much damage unit crash damage deals. (Compounds with unitDamageMultiplier)"],
//     "unitCostMultiplier": f32 ["Multiplier of resources that units take to build."],
//     "unitHealthMultiplier": f32 ["How much health units start with."],
//     "blockHealthMultiplier": f32 ["How much health blocks start with."],
//     "blockDamageMultiplier": f32 ["How much damage blocks (turrets) deal."],
//     "buildSpeedMultiplier": f32 ["Multiplier for building speed."],
// );

pub async fn commit(stdin: &broadcast::Sender<String>) {
    send!(
        stdin,
        "rules {}",
        serde_json::to_string(&*rules(stdin).await).unwrap()
    )
    .unwrap();
}

pub async fn rules(stdin: &broadcast::Sender<String>) -> tokio::sync::MutexGuard<Rules> {
    static RULES: OnceCell<Mutex<Rules>> = OnceCell::const_new();
    RULES
        .get_or_init(|| async move {
            send!(stdin, "rules").unwrap();
            let res = get_nextblock().await;
            Mutex::new(serde_json::from_str(&res).unwrap())
        })
        .await
        .lock()
        .await
}

#[poise::command(slash_command, category = "Configuration", rename = "list_rules")]
/// check them rules
pub async fn list(ctx: Context<'_>) -> Result<()> {
    poise::send_reply(
        ctx,
        poise::CreateReply::default().embed(
            CreateEmbed::new()
                .title("rules")
                .fields(
                    rules(&ctx.data().stdin)
                        .await
                        .reduce()
                        .map(|(a, b)| (a.to_string(), b.to_string(), true)),
                )
                .color(super::SUCCESS),
        ),
    )
    .await?;
    Ok(())
}

pub async fn autocomplete<'a>(
    _: Context<'a>,
    partial: &'a str,
) -> impl futures::Stream<Item = &'a str> + 'a {
    futures::stream::iter(Rules::name())
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|&x| x)
}

#[poise::command(
    slash_command,
    category = "Configuration",
    rename = "set_rule",
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
/// set a rule
pub async fn set(
    ctx: Context<'_>,
    #[description = "rule"]
    #[autocomplete = "autocomplete"]
    rule: String,
    #[description = "lol"] value: String,
) -> Result<()> {
    rules(&ctx.data().stdin).await.set(&rule, &value)?;
    commit(&ctx.data().stdin).await;
    repl!(ctx, "{OK}")?;
    Ok(())
}

#[poise::command(
    slash_command,
    category = "Configuration",
    rename = "delete_rule",
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
/// delete a rule
pub async fn del(
    ctx: Context<'_>,
    #[description = "rule"]
    #[autocomplete = "autocomplete"]
    rule: String,
) -> Result<()> {
    match rules(&ctx.data().stdin).await.delete(&rule) {
        Some(true) => repl!(ctx, "{OK} removed"),
        Some(false) => repl!(ctx, "{WARNING} rule existed, but already none"),
        None => repl!(ctx, "{CANCEL} invalid rule!"),
    }?;
    commit(&ctx.data().stdin).await;

    Ok(())
}
