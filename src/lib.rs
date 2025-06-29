use serde::{Deserialize, Serialize};
use supabase_auth::models::{AuthClient, EmailSignUpResult, User};
use worker::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SendMessage {
    channel_id: String,
    url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CreateUserParam {
    email: String,
    password: String,
}

async fn get_auth_client(ctx: RouteContext<()>) -> Result<AuthClient> {
    let project_url = ctx.env.secret("SUPABASE_URL")?.to_string();
    let api_key = ctx.env.secret("SUPABASE_API_KEY")?.to_string();
    let jwt_secret = ctx.env.secret("SUPABASE_JWT_SECRET")?.to_string();
    Ok(AuthClient::new(project_url, api_key, jwt_secret))
}

async fn validate_user(req: Request, ctx: RouteContext<()>) -> Result<Option<User>> {
    let auth_client = get_auth_client(ctx).await?;
    let token = req.headers().get("Authorization")?;
    if token.is_none() {
        return Ok(None);
    }
    let token = token.unwrap();
    if let Ok(user) = auth_client.get_user(&token).await {
        return Ok(Some(user));
    }

    Err(worker::Error::RustError("Unauthorized".to_string()))
}

fn make_error_response(message: &str, status: u16) -> Result<Response> {
    let resp = Response::builder()
        .with_status(status)
        .with_header("Content-Type", "application/json");
    if let Ok(r) = resp {
        r.from_json(&serde_json::json!({
            "error": message
        }))
    } else {
        Response::error(message, status)
    }
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let router = Router::new();
    router
        .get("/", |_, _ctx| {
            Response::ok("Send message to Discord channel")
        })
        .get("/ping", |_, _ctx| Response::ok("pong"))
        .post_async("/api/login", async move |mut req, ctx| {
            let auth_client = get_auth_client(ctx).await?;
            let body = req.json::<CreateUserParam>().await?;
            match auth_client
                .login_with_email(&body.email, &body.password)
                .await
            {
                Ok(session) => {
                    let user = session.user;
                    Response::from_json(&serde_json::json!({
                        "access_token": session.access_token,
                        "refresh_token": session.refresh_token,
                        "expires_in": session.expires_in,
                        "expires_at": session.expires_at,
                        "token_type": session.token_type,
                        "user": serde_json::json!({
                            "id": user.id,
                            "email": user.email,
                            "created_at": user.created_at,
                            "updated_at": user.updated_at,
                            "last_sign_in_at": user.last_sign_in_at,
                            "email_confirmed_at": user.email_confirmed_at,
                        }),
                    }))
                }
                Err(e) => make_error_response(&e.to_string(), 400),
            }
        })
        .post_async("/api/register", async move |mut req, ctx| {
            let auth_client = get_auth_client(ctx).await?;
            let body = req.json::<CreateUserParam>().await?;
            match auth_client
                .sign_up_with_email_and_password(&body.email, &body.password, None)
                .await
            {
                Ok(EmailSignUpResult::SessionResult(session)) => {
                    let user = session.user;
                    Response::from_json(&user)
                }
                Ok(EmailSignUpResult::ConfirmationResult(confirmation)) => {
                    Response::from_json(&serde_json::json!({
                        "id": confirmation.id,
                        "aud": confirmation.aud,
                        "role": confirmation.role,
                        "confirmation_sent_at": confirmation.confirmation_sent_at,
                        "is_anonymous": confirmation.is_anonymous,
                        "created_at": confirmation.created_at,
                        "updated_at": confirmation.updated_at,
                    }))
                }
                Err(e) => make_error_response(&e.to_string(), 500),
            }
        })
        .get_async("/api/me", async move |req, ctx| {
            let user = validate_user(req, ctx).await?;
            if user.is_none() {
                return make_error_response("Unauthorized", 401);
            }
            let user = user.unwrap();
            Response::from_json(&user)
        })
        .post_async("/api/send", async move |mut req, ctx| {
            let dc_token = ctx.env.secret("DISCORD_TOKEN")?.to_string();
            let body = req.json::<SendMessage>().await?;

            let url = format!(
                "https://discord.com/api/v10/channels/{}/messages",
                body.channel_id
            );
            let headers = Headers::new();
            headers.set("Authorization", &format!("Bot {dc_token}"))?;
            headers.set("Content-Type", "application/json")?;
            let message_body = serde_json::to_string(&serde_json::json!({
                "content": body.url
            }))?;

            let response = Fetch::Request(Request::new_with_init(
                &url,
                &RequestInit {
                    method: Method::Post,
                    headers,
                    body: Some(message_body.into()),
                    ..Default::default()
                },
            )?)
            .send()
            .await?;

            if response.status_code() < 200 || response.status_code() >= 300 {
                return make_error_response(
                    &format!("Discord API error: {}", response.status_code()),
                    response.status_code(),
                );
            }

            Response::from_json(&serde_json::json!({
                "status": "ok",
            }))
        })
        .run(req, env)
        .await
}
