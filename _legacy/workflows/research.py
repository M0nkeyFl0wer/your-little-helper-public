import sys
import json
import time

def perform_research(query):
    """
    Simulates a deep research task.
    In a production setup, this would connect to Tavily, Serper, or a browser agent.
    """
    
    # Simulate "thinking" and "searching"
    time.sleep(1)
    
    # Mock response to demonstrate the UI
    report = {
        "query": query,
        "sources": [
            {"title": "Example Source 1", "url": "https://example.com/1"},
            {"title": "Example Source 2", "url": "https://example.com/2"}
        ],
        "summary": f"This is a simulated research report for '{query}'.\n\n"
                   f"1. **Key Finding 1**: The subject '{query}' is complex and requires multiple steps to understand.\n"
                   f"2. **Key Finding 2**: Experts suggest that abstracting complexity is key to user adoption.\n\n"
                   f"Use the 'Preview' panel to see the full list of sources."
    }
    
    return report

def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "No query provided"}))
        return

    query = sys.argv[1]
    results = perform_research(query)
    print(json.dumps(results, indent=2))

if __name__ == "__main__":
    main()
