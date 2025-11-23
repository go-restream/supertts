#!/bin/bash

# Test script for improved voice style support

echo "=== superTTS Voice Style Test Script ==="
echo ""

# Function to test API endpoint
test_api() {
    local endpoint=$1
    local description=$2

    echo "Testing: $description"
    echo "Endpoint: $endpoint"

    if curl -s -f "http://localhost:8080$endpoint" > /dev/null 2>&1; then
        echo "✅ SUCCESS"
        curl -s "http://localhost:8080$endpoint" | jq '.' 2>/dev/null || curl -s "http://localhost:8080$endpoint"
    else
        echo "❌ FAILED - Make sure the API server is running on port 8080"
        echo "Start it with: cargo run --release --bin supertts -- --openai --port 8080"
    fi
    echo ""
}

echo "First, make sure the API server is running:"
echo "cargo run --release --bin supertts -- --openai --port 8080"
echo ""
echo "Press Enter to continue with tests..."
read -r

# Test health check
test_api "/health" "Health Check"

# Test voice listing
test_api "/voices" "List Available Voices"

echo "=== Voice Style Examples ==="
echo ""

echo "Example API calls for different voice styles:"
echo ""

echo "1. Standard voice name (alloy) with default model:"
echo "curl -X POST http://localhost:8080/v1/audio/speech \\"
echo "  -H 'Content-Type: application/json' \\"
echo "  -d '{\"model\": \"supertts\", \"input\": \"Hello, I am using the alloy voice style.\", \"voice\": \"f1\"}' \\"
echo "  --output f1_voice.wav"
echo ""

echo "2. OpenAI-compatible model (tts-1):"
echo "curl -X POST http://localhost:8080/v1/audio/speech \\"
echo "  -H 'Content-Type: application/json' \\"
echo "  -d '{\"model\": \"supertts\", \"input\": \"This uses OpenAI model name.\", \"voice\": \"f1\"}' \\"
echo "  --output f1_voice.wav"
echo ""

echo "3. Explicit response format:"
echo "curl -X POST http://localhost:8080/v1/audio/speech \\"
echo "  -H 'Content-Type: application/json' \\"
echo "  -d '{\"model\": \"supertts\", \"input\": \"Explicit WAV format.\", \"voice\": \"female1\", \"response_format\": \"wav\"}' \\"
echo "  --output female1_voice.wav"
echo ""

echo "4. Direct file path:"
echo "curl -X POST http://localhost:8080/v1/audio/speech \\"
echo "  -H 'Content-Type: application/json' \\"
echo "  -d '{\"model\": \"supertts\", \"input\": \"This uses a direct file path.\", \"voice\": \"assets/voice_styles/F1.json\"}' \\"
echo "  --output direct_path_voice.wav"
echo ""

echo "5. Partial match (will find 'male' in M1.json):"
echo "curl -X POST http://localhost:8080/v1/audio/speech \\"
echo "  -H 'Content-Type: application/json' \\"
echo "  -d '{\"model\": \"tts-1\", \"input\": \"Male voice detected by partial match.\", \"voice\": \"male1\"}' \\"
echo "  --output male1_voice.wav"
echo ""

echo "6. Custom voice file (if exists):"
echo "curl -X POST http://localhost:8080/v1/audio/speech \\"
echo "  -H 'Content-Type: application/json' \\"
echo "  -d '{\"model\": \"supertts\", \"input\": \"Using custom voice style.\", \"voice\": \"m2\"}' \\"
echo "  --output m2_voice.wav"
echo ""

echo "7. Error handling - unsupported format:"
echo "curl -X POST http://localhost:8080/v1/audio/speech \\"
echo "  -H 'Content-Type: application/json' \\"
echo "  -d '{\"input\": \"This will fail.\", \"voice\": \"alloy\", \"response_format\": \"mp3\"}'"
echo ""

echo "8. Error handling - non-existent voice:"
echo "curl -X POST http://localhost:8080/v1/audio/speech \\"
echo "  -H 'Content-Type: application/json' \\"
echo "  -d '{\"model\": \"supertts\", \"input\": \"This will fail.\", \"voice\": \"nonexistent_voice\"}'"
echo ""

echo "=== Test Complete ==="