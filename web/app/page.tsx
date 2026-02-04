'use client';

import { useState } from 'react';
import { Activity, HardDrive, Cpu, Terminal, Zap } from 'lucide-react';

export default function Home() {
  const [loading, setLoading] = useState(false);
  const [data, setData] = useState<any>(null);
  const [logs, setLogs] = useState<string>('');

  const runDiagnostics = async () => {
    setLoading(true);
    setLogs('Initializing diagnostics...\n');
    setData(null);

    try {
      const res = await fetch('/api/fix', { method: 'POST' });
      const result = await res.json();

      if (result.success) {
        // Parse the stdout JSON
        try {
          const parsedData = JSON.parse(result.stdout);
          setData(parsedData);
        } catch (e) {
          setLogs(prev => prev + '\n[Error] Failed to parse JSON output: ' + result.stdout);
        }
        // Append stderr to logs
        setLogs(prev => prev + '\n' + result.stderr);
      } else {
        setLogs(prev => prev + '\n[Error] ' + result.error + '\n' + result.stderr);
      }
    } catch (err) {
      setLogs(prev => prev + '\n[Fatal Error] Failed to contact API.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <main className="min-h-screen p-8 max-w-6xl mx-auto flex flex-col gap-8">
      {/* Header */}
      <header className="flex items-center justify-between">
        <h1 className="text-4xl font-bold tracking-tighter text-glow">
          Little Helper <span className="text-sm font-normal text-purple-400 opacity-70">v2.0 (Python Core)</span>
        </h1>
        <div className="flex gap-4">
          <button
            onClick={runDiagnostics}
            disabled={loading}
            className="glass px-6 py-2 rounded-full font-semibold hover:bg-white/10 transition flex items-center gap-2 disabled:opacity-50"
          >
            <Zap size={18} className={loading ? "animate-spin" : "text-purple-400"} />
            {loading ? 'Running...' : 'Run Diagnostics'}
          </button>
        </div>
      </header>

      {/* Metrics Grid */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {/* CPU */}
        <div className="glass-card">
          <div className="flex justify-between items-start mb-4">
            <h2 className="text-lg text-gray-400">CPU Load</h2>
            <Cpu className="text-purple-500" />
          </div>
          <div className="text-4xl font-mono font-bold">
            {data ? `${data.metrics.cpu.usage_percent}%` : '--'}
          </div>
          <p className="text-xs text-gray-500 mt-2">
            {data ? `${data.metrics.cpu.count} Cores Active` : 'Waiting for data...'}
          </p>
        </div>

        {/* Memory */}
        <div className="glass-card">
          <div className="flex justify-between items-start mb-4">
            <h2 className="text-lg text-gray-400">Memory</h2>
            <Activity className="text-blue-500" />
          </div>
          <div className="text-4xl font-mono font-bold">
            {data ? `${data.metrics.memory.percent}%` : '--'}
          </div>
          <p className="text-xs text-gray-500 mt-2">
            {data ? `${data.metrics.memory.available_gb}GB Available` : 'Waiting for data...'}
          </p>
        </div>

        {/* Disk */}
        <div className="glass-card">
          <div className="flex justify-between items-start mb-4">
            <h2 className="text-lg text-gray-400">Disk</h2>
            <HardDrive className="text-green-500" />
          </div>
          <div className="text-4xl font-mono font-bold">
            {data ? `${data.metrics.disk.percent}%` : '--'}
          </div>
          <p className="text-xs text-gray-500 mt-2">
            {data ? `${data.metrics.disk.free_gb}GB Free` : 'Waiting for data...'}
          </p>
        </div>
      </div>

      {/* Analysis Section */}
      {data && (
        <section className="glass-card border-purple-500/30">
          <h2 className="text-xl font-bold mb-4 flex items-center gap-2">
            <Zap className="text-yellow-400" /> AI Analysis
          </h2>
          <div className="prose prose-invert max-w-none">
            <p className="whitespace-pre-wrap text-gray-300 leading-relaxed">
              {data.analysis}
            </p>
          </div>
        </section>
      )}

      {/* Console Output */}
      <section className="glass-card bg-black/80 font-mono text-sm overflow-hidden flex flex-col h-64">
        <div className="flex items-center gap-2 border-b border-white/10 pb-2 mb-2 text-gray-500">
          <Terminal size={14} />
          <span>System Log</span>
        </div>
        <div className="overflow-y-auto flex-1 text-gray-400 whitespace-pre-wrap font-mono">
          {logs || "Ready to serve."}
        </div>
      </section>
    </main>
  );
}
