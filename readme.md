```markdown
# ChatFox

ChatFox is a simple Rust application that allows users to interact with AI models like OpenAI's GPT-3.5 and Anthropic's Claude through a chat interface.

## Features

- Communicate with OpenAI and Anthropic AI models.
- Simple chat interface served locally.
- Supports entering API keys via a `.env` file.

## Requirements

- Rust (latest stable version recommended)
- An OpenAI API Key
- An Anthropic API Key (if you wish to use Anthropic models)

## Building and Running

1. **Clone the Repository:**

   ```bash
   git clone https://github.com/nigelp/chat_fox.git
   cd chat_fox

2. Change the name of .env.example file to .env
	Add your API keys to the .env file as indicated. 
	OPENAI_API_KEY=your_openai_api_key_here
	ANTHROPIC_API_KEY=your_anthropic_api_key_here


3. Build the application:
	cargo build --release

4. Run the application:
	cargo run --release

5. Access the Chat Interface:
   Open your web browser and navigate to http://localhost:3030.
   Start chatting with the AI models!
   
Notes
    This application is designed to run on Windows but can be built and run on other platforms with minor 	adjustments.
    Ensure that you have an internet connection and valid API keys.
    Important: Do not commit your .env file to version control, as it contains sensitive API keys.

License
This project is licensed under the MIT License.

