import React, { useState, useRef, useEffect } from 'react';

interface Message {
    id: string;
    sender: 'user' | 'system';
    content: string;
    timestamp: Date;
}

interface Props {
    onSend: (msg: string) => void;
    messages?: Message[]; // Optional external messages if managed by parent
}

export const ChatPanel: React.FC<Props> = ({ onSend, messages: extMessages = [] }) => {
    const [input, setInput] = useState('');
    const [localMessages, setLocalMessages] = useState<Message[]>([]);
    const scrollRef = useRef<HTMLDivElement>(null);

    // Combine and sort messages
    const allMessages = [...localMessages, ...extMessages].sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());

    useEffect(() => {
        if (scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
        }
    }, [allMessages.length]);

    const handleSend = () => {
        if (!input.trim()) return;

        const newMsg: Message = {
            id: Date.now().toString(),
            sender: 'user',
            content: input.trim(),
            timestamp: new Date(),
        };

        // Optimistic update
        setLocalMessages(prev => [...prev, newMsg]);
        onSend(input.trim());
        setInput('');
    };

    return (
        <div className="flex flex-col h-full bg-neutral-50 dark:bg-neutral-900/30">
            {/* Messages Area */}
            <div className="flex-1 overflow-y-auto p-4 space-y-4" ref={scrollRef}>
                {allMessages.length === 0 && (
                    <div className="h-full flex flex-col items-center justify-center text-neutral-400 dark:text-neutral-600 opacity-50">
                        <svg className="w-12 h-12 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" /></svg>
                        <p className="text-sm">Start a conversation</p>
                    </div>
                )}
                {allMessages.map((msg, idx) => {
                    const isUser = msg.sender === 'user';
                    return (
                        <div key={idx} className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}>
                            <div
                                className={`max-w-[80%] px-4 py-2.5 rounded-2xl shadow-sm text-sm leading-relaxed ${isUser
                                        ? 'bg-indigo-600 text-white rounded-br-none'
                                        : 'bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-700 text-neutral-800 dark:text-neutral-200 rounded-bl-none'
                                    }`}
                            >
                                {msg.content}
                                <div className={`text-[10px] mt-1 opacity-70 ${isUser ? 'text-indigo-200' : 'text-neutral-400'}`}>
                                    {new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                                </div>
                            </div>
                        </div>
                    );
                })}
            </div>

            {/* Input Area */}
            <div className="p-4 bg-white dark:bg-neutral-800 border-t border-neutral-200 dark:border-neutral-700">
                <form
                    onSubmit={(e) => { e.preventDefault(); handleSend(); }}
                    className="flex items-center gap-2 bg-neutral-100 dark:bg-neutral-900 rounded-xl px-4 py-2 border border-transparent focus-within:border-indigo-500 transition-colors"
                >
                    <input
                        className="flex-1 bg-transparent border-none outline-none text-neutral-900 dark:text-neutral-100 placeholder-neutral-500"
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        placeholder="Type your message..."
                    />
                    <button
                        type="submit"
                        disabled={!input.trim()}
                        className="text-indigo-600 dark:text-indigo-400 hover:text-indigo-700 disabled:opacity-50 disabled:cursor-not-allowed p-1 transition-colors"
                    >
                        <svg className="w-5 h-5 rotate-90" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8" /></svg>
                    </button>
                </form>
            </div>
        </div>
    );
};
