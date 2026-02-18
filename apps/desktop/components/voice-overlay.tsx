import React from 'react';

interface Props {
    active: boolean;
    onToggle: () => void;
}

export const VoiceOverlay: React.FC<Props> = ({ active, onToggle }) => {
    return (
        <div
            className={`fixed bottom-6 right-6 w-14 h-14 rounded-full flex items-center justify-center cursor-pointer transition-all duration-300 z-50 shadow-lg hover:shadow-xl hover:scale-105 active:scale-95 ${active
                ? 'bg-rose-500 hover:bg-rose-600 shadow-rose-500/30 animate-pulse'
                : 'bg-indigo-600 hover:bg-indigo-700 shadow-indigo-500/30'
                }`}
            onClick={onToggle}
        >
            {active ? (
                <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 10a1 1 0 011-1h4a1 1 0 011 1v4a1 1 0 01-1 1h-4a1 1 0 01-1-1v-4z" />
                </svg>
            ) : (
                <svg className="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z" />
                </svg>
            )}
        </div>
    );
};
