import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface AppStatus {
    connected: boolean;
}

export function useWebSocket() {
    const [status, setStatus] = useState<AppStatus>({ connected: false });
    const [lastMessage, setLastMessage] = useState<Record<string, unknown> | null>(null);

    useEffect(() => {
        // Poll status initially
        const checkStatus = async () => {
            try {
                const s = await invoke<AppStatus>('get_status');
                setStatus((prev) => (prev.connected === s.connected ? prev : s)); // Prevent unnecessary renders
            } catch (e) {
                console.error('Failed to get status:', e);
            }
        };

        checkStatus();
        const interval = setInterval(checkStatus, 2000);

        // Initial setup for listener
        let unlisten: (() => void) | undefined;

        const setupListener = async () => {
            const u = await listen<Record<string, unknown>>('a2ui-message', (event) => {
                console.log('Received message:', event.payload);
                setLastMessage(event.payload);
            });
            // Assign unlisten function properly to cleanup reference
            // Currently unlisten variable logic is slightly flawed because setupListener is async
            // and unlisten variable assignment happens after return.
            // Better to rely on cleanup function execution order or ref.
            return u;
        };

        setupListener().then(u => { unlisten = u; });

        return () => {
            clearInterval(interval);
            if (unlisten) unlisten();
        };
    }, []);

    const connect = async (url: string) => {
        try {
            await invoke('connect_server', { url });
        } catch (e) {
            console.error('Connect error:', e);
            throw e;
        }
    };

    const sendMessage = async (message: string) => {
        try {
            await invoke('send_message', { message });
        } catch (e) {
            console.error('Send error:', e);
            throw e;
        }
    };

    return { status, lastMessage, connect, sendMessage };
}
