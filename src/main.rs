#![windows_subsystem = "windows"]
use anyhow::Result;
use reqwest::{
    header::{HeaderMap, HeaderValue},
};
use async_openai::{
    types::{ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, Role},
    config::OpenAIConfig,
    Client as OpenAIClient,
};
use dotenv::dotenv;
use std::sync::Arc;
use tokio;
use warp::{ws::Message, Filter};
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;
use futures_util::SinkExt;
use std::collections::VecDeque;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    println!("AI Chat MVP (OpenAI/Anthropic)");
    println!("Type 'switch' to toggle between OpenAI and Anthropic");
    println!("Type 'exit' to quit");
    println!("Using OpenAI by default");

    let openai_api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    // Create a configuration with the API key
    let openai_config = OpenAIConfig::new().with_api_key(&openai_api_key);
    let openai_client = OpenAIClient::with_config(openai_config);

    let anthropic_api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");
    let anthropic_client = create_anthropic_client(anthropic_api_key);

    let clients = Arc::new((openai_client, anthropic_client));

    let static_files = warp::path::end()
        .and(warp::fs::file("index.html"))
        .or(warp::fs::dir("."));

    let clients_filter = warp::any().map(move || clients.clone());

    let chat = warp::path("chat")
        .and(warp::ws())
        .and(clients_filter)
        .map(|ws: warp::ws::Ws, clients| {
            ws.on_upgrade(move |socket| handle_websocket(socket, clients))
        });

    let routes = static_files.or(chat);

    // Restore browser launching code (Windows-specific)
    let window_args = [
        "--window-size=800,600",
        "--window-position=center",
        "--app=http://localhost:3030"
    ];

    let browsers = [
        ("chrome.exe", window_args.to_vec()),
        ("msedge.exe", window_args.to_vec()),
        ("firefox.exe", vec!["--new-window", "http://localhost:3030", "--width=800", "--height=600"])
    ];

    let mut browser_launched = false;
    for (browser, args) in browsers.iter() {
        let program_files = std::env::var("ProgramFiles").unwrap_or("C:\\Program Files".to_string());
        let program_files_x86 = std::env::var("ProgramFiles(x86)").unwrap_or("C:\\Program Files (x86)".to_string());
        
        let possible_paths = vec![
            format!("{}\\Google\\Chrome\\Application\\{}", program_files, browser),
            format!("{}\\Microsoft\\Edge\\Application\\{}", program_files, browser),
            format!("{}\\Mozilla Firefox\\{}", program_files, browser),
            format!("{}\\Google\\Chrome\\Application\\{}", program_files_x86, browser),
            format!("{}\\Microsoft\\Edge\\Application\\{}", program_files_x86, browser),
            format!("{}\\Mozilla Firefox\\{}", program_files_x86, browser),
        ];

        for path in possible_paths {
            if std::path::Path::new(&path).exists() {
                if let Ok(_) = std::process::Command::new(&path).args(args).spawn() {
                    browser_launched = true;
                    break;
                }
            }
        }
        if browser_launched { break; }
    }

    println!("Server started at http://localhost:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    message: String,
    using_openai: bool,
    model: String,
}

async fn handle_websocket(
    websocket: warp::ws::WebSocket,
    clients: Arc<(OpenAIClient<OpenAIConfig>, reqwest::Client)>,
) {
    let (mut tx, mut rx) = websocket.split();

    // Initialize conversation history for Anthropic API
    let mut conversation_history = VecDeque::new();

    while let Some(result) = rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error: {}", e);
                break;
            }
        };

        if let Ok(text) = msg.to_str() {
            let chat_message: ChatMessage = match serde_json::from_str(text) {
                Ok(msg) => msg,
                Err(e) => {
                    eprintln!("Failed to parse message: {}", e);
                    continue;
                }
            };

            let response = if chat_message.using_openai {
                chat_with_openai(&clients.0, &chat_message.message, &chat_message.model).await
            } else {
                // For Anthropic, maintain conversation history
                conversation_history.push_back(("Human:".to_string(), chat_message.message.clone()));
                let history = conversation_history.clone();
                let res = chat_with_anthropic(&clients.1, &history).await;
                if let Ok(ref resp_text) = res {
                    conversation_history.push_back(("Assistant:".to_string(), resp_text.clone()));
                }
                res
            };

            if let Ok(response) = response {
                if let Err(e) = tx.send(Message::text(response)).await {
                    eprintln!("Failed to send message: {}", e);
                    break;
                }
            }
        }
    }
}

async fn chat_with_openai(client: &OpenAIClient<OpenAIConfig>, input: &str, model: &str) -> Result<String> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: Some(input.to_string()),
            name: None,
            function_call: None,
        }])
        .temperature(0.7)
        .build()?;

    let response = client.chat().create(request).await?;
    let choice = response.choices.first().ok_or_else(|| {
        anyhow::anyhow!("No response choices returned from OpenAI")
    })?;

    Ok(choice.message.content.clone().unwrap_or_default())
}

fn create_anthropic_client(api_key: String) -> reqwest::Client {
    let mut headers = HeaderMap::new();
    headers.insert("x-api-key", HeaderValue::from_str(&api_key).unwrap());
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("Anthropic-Version", HeaderValue::from_static("2023-06-01")); // Add the required header

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap()
}

async fn chat_with_anthropic(
    client: &reqwest::Client,
    history: &VecDeque<(String, String)>,
) -> Result<String> {
    // Construct the prompt as per Anthropic's API requirements
    let mut prompt = String::new();
    for (role, content) in history.iter() {
        prompt.push_str(&format!("{} {}\n\n", role, content));
    }
    prompt.push_str("Assistant:"); // Indicate that it's the assistant's turn

    // Use the specified model name
    let model_name = "claude-2"; // Updated model name for compatibility

    // Create the request body matching Anthropic's expected format
    let request_body = serde_json::json!({
        "prompt": prompt,
        "model": model_name,
        "max_tokens_to_sample": 2048,
        "temperature": 0.7,
        "stop_sequences": ["\n\nHuman:"]
    });

    // Send the request to the correct endpoint
    let response = client
        .post("https://api.anthropic.com/v1/complete")
        .json(&request_body)
        .send()
        .await;

    // Check if the request was successful
    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                // Parse the response correctly
                let response_json: serde_json::Value = resp.json().await?;
                // Extract the 'completion' field from the response
                let completion = response_json["completion"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid response format from Anthropic"))?;
                Ok(completion.trim().to_string())
            } else {
                // Read the error message from the response body
                let status = resp.status();
                let error_text = resp.text().await.unwrap_or_default();
                eprintln!("Anthropic API returned error: {} - {}", status, error_text);
                Err(anyhow::anyhow!(
                    "Anthropic API returned error: {} - {}",
                    status,
                    error_text
                ))
            }
        }
        Err(err) => {
            eprintln!("Error sending request to Anthropic API: {}", err);
            Err(anyhow::anyhow!(
                "Error sending request to Anthropic API: {}",
                err
            ))
        }
    }
}
