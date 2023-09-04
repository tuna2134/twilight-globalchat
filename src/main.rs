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

    // Specify intents requesting events about things like new and updat
    let intents = Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT | Intents::GUILDS;

    // Create a single shard.
    let mut shard = Shard::new(ShardId::ONE, token.clone(), intents);

    // The http client is separate from the gateway, so startup a new
    // one, also use Arc such that it can be cloned to other threads.
    let http = Arc::new(HttpClient::new(token));

    // Since we only care about messages, make the cache only process messages.
    let cache = Arc::new(
        InMemoryCache::builder()
            .resource_types(ResourceType::MESSAGE | ResourceType::CHANNEL)
            .build(),
    );

    // Startup the event loop to process each event in the event stream as they
    // come in.
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
        // Update the cache.
        cache.update(&event);

        // Spawn a new task to handle the event
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
                            http.execute_webhook(webhooker.id, webhooker.token.unwrap().as_str())
                                .username(&msg.author.name.clone())?
                                .content(&msg.content.clone())?
                                .avatar_url(&format!(
                                    "https://cdn.discordapp.com/avatars/{}/{}.png",
                                    msg.author.id, msg.author.avatar.unwrap().to_string()
                                ))
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
