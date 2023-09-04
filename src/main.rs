use dotenv::dotenv;
use std::{env, error::Error, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Event, Intents, Shard, ShardId};
use twilight_http::Client as HttpClient;
use twilight_model::channel::webhook::Webhook;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")?;
    
    let intents = Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT | Intents::GUILDS;

    let mut shard = Shard::new(ShardId::ONE, token.clone(), intents);
    let http = Arc::new(HttpClient::new(token));

    let cache = Arc::new(
        InMemoryCache::builder()
            .resource_types(ResourceType::MESSAGE | ResourceType::CHANNEL)
            .build(),
    );

    loop {
        let event = match shard.next_event().await {
            Ok(event) => event,
            Err(source) => {
                tracing::warn!(?source, "error receiving event");

                if source.is_fatal() {
                    break;
                }

                continue;
            }
        };
        cache.update(&event);
        tokio::spawn(handle_event(event, Arc::clone(&http), Arc::clone(&cache)));
    }

    Ok(())
}

async fn handle_event(
    event: Event,
    http: Arc<HttpClient>,
    cache: Arc<InMemoryCache>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MessageCreate(msg) => {
            if msg.author.bot {
                return Ok(());
            }
            match cache.channel(msg.channel_id) {
                Some(channel) => {
                    if channel.value().name.clone().unwrap() == "test-global" {
                        for target in cache.iter().channels() {
                            if target.id == msg.channel_id {
                                continue;
                            }
                            if target.value().name.clone().unwrap() != "test-global" {
                                continue;
                            }
                            let webhooks = http.channel_webhooks(target.id).await?;
                            let mut webhooker: Option<Webhook> = None;
                            for webhook in webhooks.model().await? {
                                if webhook.name.clone().unwrap() == "test-global".to_string() {
                                    webhooker = Some(webhook);
                                    break;
                                }
                            }
                            if webhooker.is_none() {
                                webhooker = Some(
                                    http.create_webhook(target.id, "test-global")?
                                        .await?
                                        .model()
                                        .await?,
                                );
                            }
                            let webhooker = webhooker.unwrap();
                            let mut avatar_url: String = String::new();
                            if let Some(avatar) = msg.author.avatar {
                                avatar_url = format!(
                                    "https://cdn.discordapp.com/avatars/{}/{}.png",
                                    msg.author.id, avatar.to_string()
                                )
                            }
                            http.execute_webhook(webhooker.id, webhooker.token.unwrap().as_str())
                                .username(&msg.author.name.clone())?
                                .content(&msg.content.clone())?
                                .avatar_url(&avatar_url)
                                .await?;
                        }
                    }
                }
                None => {
                    println!("Channel cannot found");
                }
            }
        }
        Event::Ready(_) => {
            println!("Shard is ready");
        }
        _ => {}
    }

    Ok(())
}
