# SuperTTS API Documentation

## Overview

SuperTTS provides a RESTful API service that converts text to speech using neural voice synthesis. The service is compatible with OpenAI's Speech API standard for seamless integration.

## Base URL

```
http://localhost:8080
```

## Endpoints

### Health Check
```
GET /health
```

Returns the service health status including version and model loading information.

#### Response

```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T12:00:00Z",
  "version": "1.0.0",
  "model_loaded": true
}
```

#### Response Fields

| Field | Type | Description |
|-------|------|-------------|
| status | string | Service status (e.g., "healthy") |
| timestamp | string | RFC3339 formatted timestamp |
| version | string | Application version |
| model_loaded | boolean | Whether the TTS model is loaded |

### List Voices
```
GET /voices
```

Retrieves a list of all available voice styles and their availability status.

#### Response

```json
{
  "voices": [
    {
      "name": "m1",
      "path": "assets/voice_styles/M1.json",
      "exists": true
    },
    {
      "name": "f1",
      "path": "assets/voice_styles/F1.json",
      "exists": true
    }
  ],
  "timestamp": "2024-01-01T12:00:00Z"
}
```

#### Response Fields

| Field | Type | Description |
|-------|------|-------------|
| voices | array | List of available voice information |
| timestamp | string | RFC3339 formatted timestamp |

#### Voice Object Fields

| Field | Type | Description |
|-------|------|-------------|
| name | string | Voice identifier (can be used in TTS requests) |
| path | string | File path to voice configuration |
| exists | boolean | Whether the voice file exists and is available |

#### Standard Voice Names

| Name | Description | File |
|------|-------------|------|
| m1, male1 | Male voice 1 | M1.json |
| m2, male2 | Male voice 2 | M2.json |
| f1, female1 | Female voice 1 | F1.json |
| f2, female2 | Female voice 2 | F2.json |

### Text-to-Speech
```
POST /v1/audio/speech
```

Converts text input into high-quality audio speech using neural voice models.

#### Request Headers

| Header | Value | Required |
|--------|-------|----------|
| Authorization | Bearer YOUR_API_KEY | Yes |
| Content-Type | application/json | Yes |

#### Request Parameters

| Parameter | Type | Description | Required | Default |
|-----------|------|-------------|----------|---------|
| input | string | Text content to convert to speech (English only) | Yes | - |
| voice | string | Voice model identifier | Yes | - |
| model | string | Model name | Yes | supertts |
| speed | number | Speech speed factor | No | 1.0 |

#### Voice Options

- `F1` - Female voice 1
- `F2` - Female voice 2
- `M1` - Male voice 1
- `M2` - Male voice 2

#### Speed Range

Recommended range: 0.9 - 1.5

## Usage Example

```bash
curl -X POST http://localhost:8080/v1/audio/speech \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "input": "The sun sets behind the mountains, painting the sky in shades of pink and orange.",
    "voice": "F1",
    "model": "supertts",
    "speed": 1.0
  }' \
  --output output.wav
```

## Configuration

The service can be configured using a `config.json` file with the following structure:

```json
{
  "onnx-dir": "assets/onnx",
  "total-step": 5,
  "voice-dir": "assets/voice_styles",
  "speed": 1.0
}
```

### Configuration Parameters

| Parameter | Description |
|-----------|-------------|
| onnx-dir | Directory path for ONNX model files |
| total-step | Total processing steps |
| voice-dir | Directory path for voice style files |
| speed | Default speech speed |

## Response

The API returns audio data in the specified output format (e.g., WAV file). The response is streamed directly as binary audio data.

## Error Handling

The API returns appropriate HTTP status codes for different scenarios:
- `200` - Success
- `400` - Bad Request (invalid parameters)
- `401` - Unauthorized (invalid API key)
- `500` - Internal Server Error

## Limitations

- Currently supports English text only
- Audio output format: WAV
- Maximum input text length depends on model configuration