'use client';

import { useState, useEffect } from "react";
import { invoke } from '@tauri-apps/api/core';
import { useWebSocket } from "@/hooks/useWebSocket";
import { A2uiRenderer, A2uiComponent } from "@/components/a2ui-renderer";
import { ChatPanel } from "@/components/chat-panel";
import { VoiceOverlay } from "@/components/voice-overlay";

// Minimal A2UI message types
interface A2uiServerMessage {
  type: string;
  component_id?: string;
  component_type?: string;
  props?: Record<string, unknown>;
  content?: string; // For chat
}

export default function Home() {
  // Client-side only check to avoid hydration mismatch if needed
  const [mounted, setMounted] = useState(false);
  const { status, lastMessage, connect, sendMessage } = useWebSocket();
  const [url, setUrl] = useState("ws://localhost:42000/ws");
  const [components, setComponents] = useState<A2uiComponent[]>([]);
  const [voiceActive, setVoiceActive] = useState(false);
  const [chatMessages, setChatMessages] = useState<Array<{ id: string, sender: 'user' | 'system', content: string, timestamp: Date }>>([]);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    setMounted(true);
  }, []);

  useEffect(() => {
    if (!lastMessage) return;

    console.log("Last message:", lastMessage);
    const m = lastMessage as unknown as A2uiServerMessage;

    // Handle Chat Messages
    if (m.type === "chat" && m.content) {
      // eslint-disable-next-line react-hooks/exhaustive-deps
      setChatMessages(prev => [...prev, {
        id: Date.now().toString(),
        sender: 'system',
        content: m.content!,
        timestamp: new Date()
      }]);
    }
    // Handle A2UI Messages
    else if (m.type === "render" && m.component_id && m.component_type && m.props) {
      // eslint-disable-next-line react-hooks/exhaustive-deps
      setComponents((prev) => [
        ...prev,
        { id: m.component_id!, type: m.component_type!, props: m.props! },
      ]);
    } else if (m.type === "update" && m.component_id) {
      // eslint-disable-next-line react-hooks/exhaustive-deps
      setComponents((prev) =>
        prev.map((c) =>
          c.id === m.component_id ? { ...c, props: { ...c.props, ...m.props } } : c
        )
      );
    } else if (m.type === "remove" && m.component_id) {
      // eslint-disable-next-line react-hooks/exhaustive-deps
      setComponents((prev) => prev.filter((c) => c.id !== m.component_id));
    }
  }, [lastMessage]);

  const handleConnect = () => {
    connect(url);
  };

  const handleChatSend = (msg: string) => {
    sendMessage(msg);
    // Add user message locally (optimistic handled in ChatPanel, but sync here if needed)
    // Actually ChatPanel handles local state.
    // Sync external messages?
  };

  const toggleVoice = async () => {
    try {
      if (voiceActive) {
        await invoke('stop_voice');
      } else {
        await invoke('start_voice', { mode: 'ptt' });
      }
      setVoiceActive(!voiceActive);
    } catch (e) {
      console.error("Voice toggle error:", e);
    }
  };

  if (!mounted) return null;

  return (
    <div className="min-h-screen bg-neutral-50 dark:bg-neutral-900 text-neutral-900 dark:text-neutral-100 flex flex-col font-[family-name:var(--font-geist-sans)]">
      {/* Header */}
      <header className="px-6 py-4 flex items-center justify-between border-b border-neutral-200 dark:border-neutral-800 bg-white dark:bg-neutral-900/80 backdrop-blur-md sticky top-0 z-50">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-lg bg-indigo-600 flex items-center justify-center shadow-indigo-500/20 shadow-lg">
            <svg className="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </div>
          <h1 className="text-xl font-bold bg-gradient-to-r from-indigo-600 to-violet-600 bg-clip-text text-transparent">
            Cratos
          </h1>
        </div>

        <div className="flex items-center gap-4">
          {/* Connection Status Indicator */}
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-neuarl-100 dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-700 text-sm">
            <div className={`w-2.5 h-2.5 rounded-full ${status.connected ? "bg-emerald-500 animate-pulse" : "bg-rose-500"}`} />
            <span className={status.connected ? "text-emerald-700 dark:text-emerald-400" : "text-rose-700 dark:text-rose-400 font-medium"}>
              {status.connected ? "Connected" : "Disconnected"}
            </span>
          </div>

          <button
            onClick={toggleVoice}
            className={`p-2 rounded-full transition-all duration-300 ${voiceActive
              ? "bg-rose-500 text-white shadow-rose-500/30 shadow-lg scale-110"
              : "bg-neutral-100 dark:bg-neutral-800 text-neutral-600 dark:text-neutral-400 hover:bg-neutral-200 dark:hover:bg-neutral-700"
              }`}
          >
            {voiceActive ? (
              <svg className="w-5 h-5 animate-pulse" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
              </svg>
            ) : (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
              </svg>
            )}
          </button>
        </div>
      </header>

      {/* Settings / Connection Panel (Collapsible or visible when disconnected) */}
      {!status.connected && (
        <div className="bg-rose-50 dark:bg-rose-900/10 border-b border-rose-100 dark:border-rose-900/30 p-4">
          <div className="max-w-4xl mx-auto flex items-center justify-center gap-3">
            <input
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="ws://localhost:42000/ws"
              className="px-4 py-2 border border-neutral-300 dark:border-neutral-700 rounded-lg w-80 bg-white dark:bg-neutral-900 focus:ring-2 focus:ring-indigo-500 outline-none transition-all"
            />
            <button
              onClick={handleConnect}
              className="px-6 py-2 bg-indigo-600 hover:bg-indigo-700 text-white font-medium rounded-lg shadow-lg shadow-indigo-500/20 transition-all active:scale-95"
            >
              Connect Server
            </button>
          </div>
        </div>
      )}

      {/* Main Content Area */}
      <main className="flex-1 p-6 max-w-7xl mx-auto w-full grid grid-cols-1 lg:grid-cols-12 gap-6 h-[calc(100vh-80px)]">

        {/* Left: Chat Interface */}
        <div className="lg:col-span-5 flex flex-col h-full bg-white dark:bg-neutral-800 rounded-2xl border border-neutral-200 dark:border-neutral-700 shadow-sm overflow-hidden">
          <div className="p-4 border-b border-neutral-100 dark:border-neutral-700 bg-neutral-50/50 dark:bg-neutral-800/50 backdrop-blur-sm">
            <h2 className="font-semibold text-neutral-800 dark:text-neutral-200 flex items-center gap-2">
              <svg className="w-5 h-5 text-indigo-500" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" /></svg>
              Alliance Chat
            </h2>
          </div>
          <div className="flex-1 overflow-hidden">
            <ChatPanel onSend={handleChatSend} messages={chatMessages} />
          </div>
        </div>

        {/* Right: A2UI & Visuals */}
        <div className="lg:col-span-7 flex flex-col gap-6 h-full overflow-y-auto">
          {/* A2UI Container */}
          <div className="flex-1 bg-white dark:bg-neutral-800 rounded-2xl border border-neutral-200 dark:border-neutral-700 shadow-sm p-6 relative overflow-hidden group">
            <div className="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-teal-400 to-emerald-500 opacity-0 group-hover:opacity-100 transition-opacity" />

            <div className="flex items-center justify-between mb-6">
              <h2 className="font-semibold text-neutral-800 dark:text-neutral-200 flex items-center gap-2">
                <svg className="w-5 h-5 text-emerald-500" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" /></svg>
                Generative Interface
              </h2>
              {components.length > 0 && (
                <span className="text-xs bg-neutral-100 dark:bg-neutral-700 text-neutral-500 dark:text-neutral-400 px-2 py-1 rounded-md">
                  {components.length} Active Elements
                </span>
              )}
            </div>

            <div className="min-h-[400px] flex items-center justify-center bg-neutral-50 dark:bg-neutral-900/50 rounded-xl border-2 border-dashed border-neutral-200 dark:border-neutral-700/50">
              {components.length === 0 ? (
                <div className="text-center text-neutral-400 dark:text-neutral-600">
                  <p className="mb-2">Waiting for stream...</p>
                  <p className="text-sm opacity-60">Visual interfaces generated by Cratos will appear here.</p>
                </div>
              ) : (
                <div className="w-full h-full p-4 overflow-y-auto">
                  <A2uiRenderer
                    components={components}
                    onEvent={(id, type, payload) => sendMessage(JSON.stringify({ type: 'event', id, event: type, payload }))}
                  />
                </div>
              )}
            </div>
          </div>
        </div>
      </main>

      <VoiceOverlay active={voiceActive} onToggle={toggleVoice} />
    </div>
  );
}
