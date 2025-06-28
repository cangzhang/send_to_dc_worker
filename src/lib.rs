use worker::*;
use serde::{Deserialize, Serialize};
// use serde_json::to_string;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SendMessage {
    channel_id: String,
    url: String,
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let router = Router::new();
    router
        .get("/ping", |_, _ctx| Response::ok("pong"))
        .post_async("/send", async move |mut req, ctx| {
            let dc_token = ctx.env.secret("DISCORD_TOKEN")?.to_string();
            let body = req.json::<SendMessage>().await?;

            let url = format!("https://discord.com/api/v10/channels/{}/messages", body.channel_id);            
            let headers = Headers::new();
            headers.set("Authorization", &format!("Bot {dc_token}"))?;
            headers.set("Content-Type", "application/json")?;
            let message_body = serde_json::to_string(&serde_json::json!({
                "content": body.url
            }))?;

            let response = Fetch::Request(Request::new_with_init(&url, &RequestInit {
                method: Method::Post,
                headers,
                body: Some(message_body.into()),
                ..Default::default()
            })?).send().await?;

            if response.status_code() < 200 || response.status_code() >= 300 {
                return Err(worker::Error::RustError(format!("Discord API error: {}", response.status_code())));
            }

            Response::from_json(&serde_json::json!({
                "status": "ok",
            }))
        })
        .run(req, env)
        .await
}
