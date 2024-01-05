use super::{get_nextblock, send_ctx, Context};
use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

#[derive(poise::ChoiceParameter)]
pub enum Team {
    #[name = "survivors"]
    #[name_localized("it", "sopravvissuto")]
    #[name_localized("vi", "người sống sót")]
    Survivor,
    #[name = "infected"]
    #[name_localized("it", "infetto")]
    #[name_localized("vi", "bị lây nhiễm")]
    Infected,
}

#[poise::command(slash_command, category = "Info")]
/// show leaderboard!
pub async fn lb(
    c: Context<'_>,
    #[description = "the team to get the leaderboard of"] team: Team,
) -> Result<()> {
    send_ctx!(
        c,
        "lb {}",
        match team {
            Team::Survivor => "surv",
            Team::Infected => "inf",
        }
    )
    .unwrap();
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("[0-9]: <(.)([0-3])> ([^:]+): ([0-9]+) wins").unwrap());
    c.reply(emoji::mindustry::to_discord(&RE.replace_all(
        &get_nextblock().await[14..],
        "<$1$2> $3: $4 wins",
    )))
    .await?;
    Ok(())
}
