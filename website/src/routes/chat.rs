use maud::{DOCTYPE, html};
use maudit::route::prelude::*;

const DISCORD_URL: &str = "https://discord.com/invite/84pd4QtmzA";

#[route("/chat/")]
pub struct Chat;

impl Route for Chat {
    fn render(&self, _ctx: &mut PageContext) -> impl Into<RenderResult> {
        html! {
            (DOCTYPE)
            html lang="en" {
                head {
                    meta charset="utf-8";
                    meta http-equiv="refresh" content=(format!("0; url={DISCORD_URL}"));
                    link rel="canonical" href=(DISCORD_URL);
                    title { "Redirecting to Discord…" }
                }
                body {
                    p { "Redirecting to " a href=(DISCORD_URL) { "Discord" } "…" }
                }
            }
        }
    }
}
