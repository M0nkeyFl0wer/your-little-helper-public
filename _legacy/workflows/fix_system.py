import sys
import json
import os

# Add the parent directory to sys.path so we can import 'utils'
# regardless of where this script is run from
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from workflows.utils.system import get_system_health
from workflows.utils.llm import LLM

def main():
    """
    Diagnose system health.
    1. Gather Metrics
    2. Ask LLM for a plain-english summary/fix
    """
    print("--- 🔍 Diagnosing System Health ---", file=sys.stderr)
    
    # 1. Get Metrics
    health_data = get_system_health()
    
    # 2. Prepare Prompt
    system_prompt = "You are a helpful IT System Administrator. Analyze the following system metrics and suggest fixes if anything looks unhealthy (high CPU, RAM, or disk usage). Be concise."
    user_prompt = f"System Metrics JSON:\n{json.dumps(health_data, indent=2)}\n\nPlease diagnose the system status."

    # 3. Call LLM
    # Note: In the future, we can route this to 'seshat' if local inference is too heavy.
    llm = LLM()
    print("--- 🧠 Consulting Intelligence ---", file=sys.stderr)
    analysis = llm.complete(user_prompt, system_prompt)
    
    # 4. Output Result (JSON for UI, Text for Terminal)
    output = {
        "metrics": health_data,
        "analysis": analysis
    }
    
    # When running in "API Mode" (future), we just dump JSON.
    # For now, print friendly text for the terminal user to STDERR so it doesn't corrupt pipe
    print("\n" + "="*40, file=sys.stderr)
    print("SYSTEM HEALTH REPORT", file=sys.stderr)
    print("="*40, file=sys.stderr)
    print(analysis, file=sys.stderr)
    print("="*40, file=sys.stderr)
    
    # Dump raw JSON to stdout for the UI
    print(json.dumps(output, indent=2))

if __name__ == "__main__":
    main()
