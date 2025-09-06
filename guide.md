# ollama-proxy
this is a proxy service to wrap local ollama request with a basic auth header and https to remote ollama auth-required service

## proxy example
localhost app -> `http://localhost:11434` -> proxy -> `https://user:pass@remote.machine:11434` -> remote ollama