import React from 'react';

export interface A2uiComponent {
    id: string;
    type: string;
    props: Record<string, unknown>;
}

interface Props {
    components: A2uiComponent[];
    onEvent?: (componentId: string, eventType: string, payload: Record<string, unknown>) => void;
}

export const A2uiRenderer: React.FC<Props> = ({ components, onEvent }) => {
    if (components.length === 0) {
        return null; // Parent handles empty state
    }

    const renderComponent = (comp: A2uiComponent) => {
        const { type, id, props } = comp;
        const commonClasses = "transition-all duration-200";

        switch (type) {
            case 'card':
                return (
                    <div key={id} className={`bg-white dark:bg-neutral-800 border border-neutral-200 dark:border-neutral-700 rounded-xl p-6 shadow-sm hover:shadow-md ${commonClasses}`}>
                        {(props.title as string) && <h3 className="font-semibold text-lg text-neutral-900 dark:text-neutral-100 mb-2">{(props.title as string)}</h3>}
                        {(props.description as string) && <p className="text-neutral-600 dark:text-neutral-400 mb-4">{(props.description as string)}</p>}
                        {(props.children as A2uiComponent[]) && <A2uiRenderer components={(props.children as A2uiComponent[])} onEvent={onEvent} />}
                        {(props.action as string) && (
                            <button
                                onClick={() => onEvent?.(id, 'action', { action: props.action })}
                                className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white text-sm font-medium rounded-lg"
                            >
                                {(props.actionLabel as string) || "Action"}
                            </button>
                        )}
                    </div>
                );

            case 'button':
                return (
                    <button
                        key={id}
                        onClick={() => onEvent?.(id, 'click', {})}
                        className={`px-4 py-2 bg-emerald-600 hover:bg-emerald-700 text-white font-medium rounded-lg shadow-sm active:scale-95 ${commonClasses}`}
                    >
                        {(props.label as string) || "Click Me"}
                    </button>
                );

            case 'text':
                return (
                    <div key={id} className={`text-neutral-800 dark:text-neutral-200 ${props.bold ? 'font-bold' : ''} ${props.size === 'lg' ? 'text-lg' : 'text-base'}`}>
                        {(props.content as string) || (props.text as string)}
                    </div>
                );

            case 'image':
                return (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                        key={id}
                        src={props.src as string}
                        alt={(props.alt as string) || "A2UI Image"}
                        className="rounded-xl border border-neutral-200 dark:border-neutral-700 shadow-sm max-w-full h-auto"
                    />
                );

            default:
                // Fallback for unknown types (Generic Debug View)
                return (
                    <div key={id} className="border border-yellow-200 dark:border-yellow-900/30 bg-yellow-50 dark:bg-yellow-900/10 p-4 rounded-lg">
                        <div className="flex justify-between items-center mb-2">
                            <span className="text-xs font-mono font-semibold text-yellow-800 dark:text-yellow-500 uppercase tracking-wide">{type}</span>
                            <span className="text-xs font-mono text-neutral-400">{id}</span>
                        </div>
                        <pre className="text-xs text-neutral-600 dark:text-neutral-400 overflow-x-auto p-2 bg-white dark:bg-neutral-900 rounded border border-neutral-100 dark:border-neutral-800">
                            {JSON.stringify(props, null, 2)}
                        </pre>
                    </div>
                );
        }
    };

    return (
        <div className="space-y-4">
            {components.map(renderComponent)}
        </div>
    );
};
