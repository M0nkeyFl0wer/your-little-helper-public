import os
import json
import requests
import subprocess
from typing import Optional, Dict, Any

class LLM:
    """
    Simple interface for calling LLMs.
    Defaults to calling a local model via Ollama if no API key is present,
    or can be configured for Claude/Gemini.
    """
    
    def __init__(self, model: str = "llama3"):
        self.model = model
        # Placeholder for API keys if we add them later
        self.api_key = os.getenv("LLM_API_KEY") 

    def complete(self, prompt: str, system_prompt: Optional[str] = None) -> str:
        """
        Generate a completion.
        Currently defaults to using `ollama` CLI or HTTP API for local execution
        to keep it "Antigravity Lite".
        """
        # 1. Try Ollama (common for this user context)
        try:
            payload = {
                "model": self.model,
                "prompt": prompt,
                "stream": False
            }
            if system_prompt:
                payload["system"] = system_prompt

            # Attempt local ollama curl
            response = requests.post("http://localhost:11434/api/generate", json=payload, timeout=60)
            response.raise_for_status()
            return response.json().get("response", "")
            
        except (requests.exceptions.ConnectionError, requests.exceptions.Timeout, requests.exceptions.HTTPError) as e:
            # Fallback: Mock response if no local LLM is running or errors occur
            return f"[LLM Mock Output] (System returned {str(e)}). I received your request. System appears stable. (Install Ollama/llama3 to get real insights)."

    def chat(self, messages: list) -> str:
        # TODO: Implement chat history support
        pass
