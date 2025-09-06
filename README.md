# ollama-proxy
this is a proxy service to wrap local ollama request with a basic auth header and https to remote ollama auth-required service

I created this to call ollama on my remote server to work for me on my local laptop (for example JetBrains AI assistant)

## proxy example
localhost app -> `http://localhost:11434` -> proxy -> `https://user:pass@remote.machine:11434` -> remote ollama

## installation


### using scoop
```powershell
scoop install https://github.com/k88936/scoop-bucket/raw/refs/heads/master/bucket/ollama-proxy.json
```
### Manually
1. [NSSM (Non-Sucking Service Manager)](https://nssm.cc/download) - Download and install NSSM
2. Build the project: `cargo build --release`
3. Run `install.ps1` as administrator:

