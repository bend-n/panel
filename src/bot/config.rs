use super::{return_next, send_ctx, Context, Result};
use convert_case::{Case, Casing};
use futures_util::StreamExt;

const ITEMS: &[&str] = &[
    "desc",
    "name",
    "port",
    "autoUpdate",
    "showConnectMessages",
    "enableVotekick",
    "startCommands",
    "logging",
    "strict",
    "antiSpam",
    "interactRateWindow",
    "interactRateLimit",
    "interactRateKick",
    "messageRateLimit",
    "messageSpamKick",
    "packetSpamLimit",
    "chatSpamLimit",
    "socketInput",
    "socketInputPort",
    "socketInputAddress",
    "allowCustomClients",
    "whitelist",
    "motd",
    "autosave",
    "autosaveAmount",
    "debug",
    "snapshotInterval",
    "autoPause",
];

async fn complete<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> impl futures::Stream<Item = String> + 'a {
    futures::stream::iter(ITEMS)
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|name| name.from_case(Case::Camel).to_case(Case::Lower))
}

#[poise::command(
    slash_command,
    category = "Configuration",
    rename = "config",
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
/// change a setting
pub async fn set(
    ctx: Context<'_>,
    #[autocomplete = "complete"]
    #[description = "setting to change"]
    setting: String,
    #[description = "the value"] config: String,
) -> Result<()> {
    let setting = setting.from_case(Case::Lower).to_case(Case::Camel);
    send_ctx!(ctx, "config {setting} {config}")?;
    return_next!(ctx)
}
// TODO: config::list
