# ollama-proxy
this is a proxy service to wrap local ollama request with a basic auth header and https to remote ollama auth-required service

I created this to call ollama on my remote server to work for me on my local laptop (for example JetBrains AI assistant)

## proxy example

```mermaid
sequenceDiagram
	autonumber
	participant A as Local App<br/>(JetBrains AI, etc.)
	participant P as ollama-proxy
	participant RO as Remote Ollama
	participant OA as OpenAI-compatible<br/>LLM Service

	rect rgb(235,235,235)
	note over A,P: Local network (plain HTTP to proxy)
	A->>P: HTTP request (e.g. POST /api/chat)
	end

	alt Remote self-hosted Ollama
		note over P,RO: Encrypted + Basic Auth
		P->>RO: HTTPS + Basic Auth (user:pass@remote.machine:11434)
		RO-->>P: JSON stream / response
	else OpenAI-compatible provider
		note over P,OA: HTTPS + Bearer / API Key
		P->>OA: HTTPS + Authorization: Bearer <secret>
		OA-->>P: JSON / stream
	end

	P-->>A: Proxied response (mirrors upstream schema)
```

Flow (simplified):

1. Local app always targets `http://localhost:11434` (no auth, HTTP).
2. `ollama-proxy` upgrades outbound leg to HTTPS and injects the appropriate auth (Basic or Bearer/API key).
3. Response / streaming tokens are passed straight through to the local client.

Supported modes:
- Remote native Ollama (basic auth + HTTPS)
- OpenAI-compatible endpoints (standard OpenAI REST / streaming format)

## installation


### using scoop
```powershell
scoop install https://github.com/k88936/scoop-bucket/raw/refs/heads/master/bucket/ollama-proxy.json
```
### Manually
1. [NSSM (Non-Sucking Service Manager)](https://nssm.cc/download) - Download and install NSSM
2. Build the project: `cargo build --release`
3. Run `install.ps1` as administrator:

