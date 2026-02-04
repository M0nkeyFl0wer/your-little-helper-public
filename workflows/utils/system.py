import psutil
import platform
from datetime import datetime
from typing import Dict, Any

def get_system_health() -> Dict[str, Any]:
    """
    Gather system health metrics in a structured JSON format.
    """
    
    # cpu
    cpu_percent = psutil.cpu_percent(interval=1)
    
    # memory
    mem = psutil.virtual_memory()
    
    # disk
    disk_usage = psutil.disk_usage('/')
    
    # heavy processes
    processes = []
    for proc in psutil.process_iter(['pid', 'name', 'cpu_percent', 'memory_percent']):
        try:
            # specialized for "fright-free" user: filter out system fluff if possible,
            # but for now just get the heaviest ones
            pInfo = proc.info
            if pInfo['cpu_percent'] > 1.0 or pInfo['memory_percent'] > 1.0:
                processes.append(pInfo)
        except (psutil.NoSuchProcess, psutil.AccessDenied, psutil.ZombieProcess):
            pass
            
    # sort by CPU usage
    processes.sort(key=lambda x: x['cpu_percent'], reverse=True)
    top_processes = processes[:5]

    return {
        "timestamp": datetime.now().isoformat(),
        "os": f"{platform.system()} {platform.release()}",
        "cpu": {
            "usage_percent": cpu_percent,
            "count": psutil.cpu_count()
        },
        "memory": {
            "total_gb": round(mem.total / (1024**3), 2),
            "available_gb": round(mem.available / (1024**3), 2),
            "percent": mem.percent
        },
        "disk": {
            "total_gb": round(disk_usage.total / (1024**3), 2),
            "free_gb": round(disk_usage.free / (1024**3), 2),
            "percent": disk_usage.percent
        },
        "top_processes": top_processes
    }
