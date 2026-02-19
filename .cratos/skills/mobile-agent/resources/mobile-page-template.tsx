"use client"

import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

// --- TYPES ---
interface MobilePageProps {
    title?: string;
}

// --- ICONS (SVG Direct - No Lucide Dependency) ---
const Icons = {
    Back: () => (
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="m15 18-6-6 6-6" />
        </svg>
    ),
    Settings: () => (
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.47a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
            <circle cx="12" cy="12" r="3" />
        </svg>
    ),
    Scan: () => (
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 7V5a2 2 0 0 1 2-2h2" />
            <path d="M17 3h2a2 2 0 0 1 2 2v2" />
            <path d="M21 17v2a2 2 0 0 1-2 2h-2" />
            <path d="M7 21H5a2 2 0 0 1-2-2v-2" />
        </svg>
    )
};

// --- UTILS ---
// Simple conditional class combiner without `clsx` dependency
const cn = (...classes: (string | undefined | null | false)[]) => classes.filter(Boolean).join(' ');

// --- COMPONENT ---
export default function MobilePageTemplate({ title = "Mobile View" }: MobilePageProps) {
    const [safeArea, setSafeArea] = useState({ top: 0, bottom: 0 });
    const [platform, setPlatform] = useState<string>('unknown');

    useEffect(() => {
        // Detect Platform (Simple check)
        const isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent);
        setPlatform(isIOS ? 'ios' : 'android');

        // Simulate Safe Area detection (In real app, use CSS variables env(safe-area-inset-*))
        // This state is just for demo logic if needed
    }, []);

    const handleAction = async () => {
        try {
            // Example: Haptic Feedback using invoke (if plugin installed)
            // await invoke('plugin:haptics|vibrate');
            console.log("Action triggered");
        } catch (e) {
            console.error("Action failed", e);
        }
    };

    return (
        <div className="flex flex-col h-screen bg-neutral-50 dark:bg-neutral-950 text-neutral-900 dark:text-neutral-50 selection:bg-indigo-500/30">

            {/* 1. Status Bar / Header Area */}
            {/* Using safe-area-inset-top for notch support */}
            <header className="sticky top-0 z-50 bg-white/80 dark:bg-neutral-900/80 backdrop-blur-md border-b border-neutral-200 dark:border-neutral-800 pt-[env(safe-area-inset-top)]">
                <div className="flex items-center justify-between px-4 h-14">
                    <button
                        className="p-2 -ml-2 rounded-full hover:bg-neutral-100 dark:hover:bg-neutral-800 active:scale-95 transition-all text-neutral-600 dark:text-neutral-400"
                        onClick={() => window.history.back()}
                    >
                        <Icons.Back />
                    </button>

                    <h1 className="font-semibold text-lg truncate">{title}</h1>

                    <button
                        className="p-2 -mr-2 rounded-full hover:bg-neutral-100 dark:hover:bg-neutral-800 active:scale-95 transition-all text-neutral-600 dark:text-neutral-400"
                    >
                        <Icons.Settings />
                    </button>
                </div>
            </header>

            {/* 2. Main Content (Scrollable) */}
            <main className="flex-1 overflow-y-auto px-4 py-6 space-y-6 overscroll-contain">
                {/* Card Component */}
                <div className="bg-white dark:bg-neutral-900 rounded-2xl p-5 shadow-sm border border-neutral-100 dark:border-neutral-800 active:scale-[0.99] transition-transform duration-200 touch-pan-y">
                    <div className="flex items-center space-x-4 mb-4">
                        <div className="w-12 h-12 bg-indigo-100 dark:bg-indigo-900/30 rounded-full flex items-center justify-center text-indigo-600 dark:text-indigo-400">
                            <Icons.Scan />
                        </div>
                        <div>
                            <h3 className="font-semibold text-base">Quick Action</h3>
                            <p className="text-sm text-neutral-500 dark:text-neutral-400">Tap to start</p>
                        </div>
                    </div>

                    <button
                        onClick={handleAction}
                        className="w-full py-3 bg-indigo-600 active:bg-indigo-700 text-white font-medium rounded-xl shadow-indigo-200 dark:shadow-none transition-all touch-manipulation"
                    >
                        Start Process
                    </button>
                </div>

                {/* List Section */}
                <section>
                    <h2 className="text-sm font-semibold text-neutral-500 dark:text-neutral-500 uppercase tracking-wider mb-3 px-1">
                        Recent Activity
                    </h2>
                    <div className="bg-white dark:bg-neutral-900 rounded-2xl overflow-hidden divide-y divide-neutral-100 dark:divide-neutral-800 border border-neutral-100 dark:border-neutral-800">
                        {[1, 2, 3].map((i) => (
                            <div key={i} className="flex items-center justify-between p-4 active:bg-neutral-50 dark:active:bg-neutral-800 transition-colors">
                                <div className="flex flex-col">
                                    <span className="font-medium text-sm">Activity Item {i}</span>
                                    <span className="text-xs text-neutral-400">2 mins ago</span>
                                </div>
                                <div className="w-2 h-2 rounded-full bg-emerald-500" />
                            </div>
                        ))}
                    </div>
                </section>
            </main>

            {/* 3. Bottom Safe Area Spacer (if needed explicitly, though padding-bottom usually handles it) */}
            <div className="h-[env(safe-area-inset-bottom)] bg-white dark:bg-neutral-900" />
        </div>
    );
}
