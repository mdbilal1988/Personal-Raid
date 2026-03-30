import { useState, useEffect } from "react";
// import { invoke } from "@tauri-apps/api/core";
import "./index.css";

function App() {
  const [quorumK] = useState(3);
  const [quorumN] = useState(5);
  const [onlineNodes, setOnlineNodes] = useState(4);
  const [bandwidthUsage, setBandwidthUsage] = useState<number[]>(Array(20).fill(0));

  // Mocking real-time updates for demonstration
  useEffect(() => {
    const interval = setInterval(() => {
      // Simulate bandwidth fluctuation
      setBandwidthUsage((prev) => {
        const newUsage = Math.random() * 1000; // up to ~1MB/s
        return [...prev.slice(1), newUsage];
      });
      // Simulate node connections
      if (Math.random() > 0.9) {
         setOnlineNodes(prev => {
             const change = Math.random() > 0.5 ? 1 : -1;
             return Math.max(0, Math.min(quorumN, prev + change));
         });
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [quorumN]);

  const isAlarm = onlineNodes < quorumK;

  // Radial Health Graph calculation
  const percentage = Math.min((onlineNodes / quorumN) * 100, 100);
  const strokeDasharray = `${percentage} 100`;

  return (
    <div className="min-h-screen bg-zinc-950 text-slate-100 flex flex-col items-center p-8 font-sans">
      <h1 className="text-4xl font-bold mb-2 tracking-tight">ShardNet Dashboard</h1>
      <p className="text-zinc-400 mb-10">Elastic Decentralized Storage</p>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-8 w-full max-w-4xl">

        {/* Radial Health Graph */}
        <div className={`p-8 rounded-2xl border ${isAlarm ? 'bg-red-950/20 border-red-800 shadow-[0_0_30px_rgba(153,27,27,0.4)] animate-pulse' : 'bg-zinc-900 border-zinc-800'} flex flex-col items-center justify-center transition-all duration-500`}>
          <h2 className="text-xl font-semibold mb-6 text-zinc-300">Quorum Health</h2>

          <div className="relative w-48 h-48 flex items-center justify-center">
            <svg viewBox="0 0 36 36" className="w-full h-full transform -rotate-90">
              <path
                className="text-zinc-800"
                strokeWidth="3"
                stroke="currentColor"
                fill="none"
                d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
              />
              <path
                className={`${isAlarm ? 'text-red-500' : 'text-emerald-500'} transition-all duration-1000 ease-out`}
                strokeDasharray={strokeDasharray}
                strokeWidth="3"
                strokeLinecap="round"
                stroke="currentColor"
                fill="none"
                d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
              />
            </svg>
            <div className="absolute flex flex-col items-center">
              <span className={`text-5xl font-bold tracking-tighter ${isAlarm ? 'text-red-400' : 'text-emerald-400'}`}>
                {onlineNodes}/{quorumN}
              </span>
              <span className="text-sm text-zinc-400 mt-1 uppercase tracking-widest font-semibold">Nodes</span>
            </div>
          </div>

          <div className="mt-8 flex items-center space-x-2">
            <span className="text-sm text-zinc-400">Recovery Threshold (K):</span>
            <span className="bg-zinc-800 px-3 py-1 rounded text-zinc-200 font-bold">{quorumK}</span>
          </div>
        </div>

        {/* Live Bandwidth Monitor */}
        <div className="p-8 rounded-2xl border bg-zinc-900 border-zinc-800 flex flex-col items-center justify-center relative overflow-hidden">
          <h2 className="text-xl font-semibold mb-6 text-zinc-300 z-10">Live Bandwidth (Sync)</h2>

          <div className="w-full h-48 relative flex items-end">
            <svg className="w-full h-full absolute inset-0" preserveAspectRatio="none">
              <defs>
                <linearGradient id="gradient" x1="0%" y1="0%" x2="0%" y2="100%">
                  <stop offset="0%" stopColor="#3b82f6" stopOpacity="0.4" />
                  <stop offset="100%" stopColor="#3b82f6" stopOpacity="0" />
                </linearGradient>
              </defs>
              {/* Line and Area */}
              <path
                d={`M 0 100 ${bandwidthUsage.map((val, i) => `L ${(i / (bandwidthUsage.length - 1)) * 100} ${100 - (val / 1000) * 100}`).join(' ')} L 100 100 Z`}
                fill="url(#gradient)"
                className="transition-all duration-300"
              />
              <polyline
                points={bandwidthUsage.map((val, i) => `${(i / (bandwidthUsage.length - 1)) * 100},${100 - (val / 1000) * 100}`).join(' ')}
                fill="none"
                stroke="#3b82f6"
                strokeWidth="2"
                className="transition-all duration-300 vector-effect-non-scaling-stroke"
              />
            </svg>
          </div>

          <div className="mt-6 flex justify-between w-full text-sm text-zinc-500 font-mono z-10">
            <span>0 KB/s</span>
            <span>Limit: 1 MB/s</span>
          </div>
          <div className="absolute top-8 right-8 text-blue-400 font-mono font-bold text-xl drop-shadow-md">
            {Math.round(bandwidthUsage[bandwidthUsage.length - 1])} KB/s
          </div>
        </div>

      </div>
    </div>
  );
}

export default App;
