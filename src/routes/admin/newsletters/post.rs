use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::error_chain_fmt;
use crate::utils::{e400, e500, see_other};
use actix_web::http::header::HeaderValue;
use actix_web::http::{header, StatusCode};
use actix_web::{web, HttpResponse, ResponseError};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::{PgPool};
use std::fmt::Formatter;
use actix_web::web::ReqData;
use crate::authentication::Userid;
use crate::idempotency::{IdempotencyKey, NextAction, save_response, try_processing};

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    html_content: String,
    text_content: String,
    idempotency_key: String
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
        }
    }
}

#[tracing::instrument(name = "Publish a newsletter issue", skip_all)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    user_id: ReqData<Userid>
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let FormData{ title, html_content, text_content, idempotency_key } = form.0;
    let idempotency_key:IdempotencyKey = idempotency_key.try_into().map_err(e400)?;

    match try_processing(&pool, &idempotency_key, *user_id).await.map_err(e500)? {
        NextAction::StartProcessing => {}
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response);
        }
    }

    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &title,
                        &html_content,
                        &text_content,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    }).map_err(e500)?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain=?error,
                    "Skipping a confirmed subscriber. \
                    Their stored contact details are invalid",
                );
            }
        }
    }
    success_message().send();
    let response = see_other("/admin/newsletters");
    let response = save_response(&pool, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;

    Ok(response)
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been published!")
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers = sqlx::query!(
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| match SubscriberEmail::parse(r.email) {
        Ok(email) => Ok(ConfirmedSubscriber { email }),
        Err(error) => Err(anyhow::anyhow!(error)),
    })
    .collect();
    Ok(confirmed_subscribers)
}
