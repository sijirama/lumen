//INFO: Cat view 🐈 — the chat widget, full-size, living inside the main window.
//      Same persistent conversation as the overlay (same backend command + the
//      same broadcast events), just with room to breathe. Layout/chrome here is
//      Tailwind; bubbles/traces come from ChatKit so this and the overlay never
//      drift. (First migrated-to-Tailwind surface — see tailwind.config.js for
//      how the design tokens are bridged into utilities.)

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Send, Square, ArrowDown } from 'lucide-react';
import {
    MessageBubble, ThinkingBubble,
    type ChatMessage, type SendMessageResponse,
} from '../components/ChatKit';

const QUICK_ACTIONS = [
    "What's on my calendar today?",
    "Check my unread emails",
    "What did we discuss last time?",
    "Open my daily note",
];

function CatView() {
    const [messages, setMessages] = useState<ChatMessage[]>([]);
    const [inputValue, setInputValue] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [isThinking, setIsThinking] = useState(false);
    const [toolStatus, setToolStatus] = useState<string[]>([]);
    const [error, setError] = useState<string | null>(null);
    const [showScrollDown, setShowScrollDown] = useState(false);

    const scrollRef = useRef<HTMLDivElement>(null);
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLTextAreaElement>(null);
    const isFirstLoad = useRef(true);

    const scrollToBottom = (instant = false) => {
        const performScroll = () =>
            messagesEndRef.current?.scrollIntoView({ behavior: instant ? 'instant' : 'smooth', block: 'end' });
        if (instant) requestAnimationFrame(performScroll);
        else setTimeout(performScroll, 50);
    };

    //INFO: Show the jump-to-bottom pill only when the user has scrolled up.
    const handleScroll = () => {
        const el = scrollRef.current;
        if (!el) return;
        const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
        setShowScrollDown(distanceFromBottom > 240);
    };

    async function loadChatHistory() {
        try {
            const history = await invoke<ChatMessage[]>('get_chat_history', { sessionId: null, limit: 50 });
            setMessages(history);
            scrollToBottom(true);
        } catch (err) {
            console.error('Failed to load chat history:', err);
            setError('Failed to load chat history.');
        }
    }

    useEffect(() => {
        loadChatHistory();
        requestAnimationFrame(() => inputRef.current?.focus());
    }, []);

    //INFO: Same streaming/tool events the overlay listens to — emitted app-wide,
    //      so the cat view stays in lockstep with the one persistent chat.
    useEffect(() => {
        let unlistenTurn: (() => void) | null = null;
        let unlistenClear: (() => void) | null = null;
        let unlistenMsg: (() => void) | null = null;
        let unlistenToolStart: (() => void) | null = null;
        let unlistenToolEnd: (() => void) | null = null;

        async function setup() {
            const { listen } = await import('@tauri-apps/api/event');

            unlistenTurn = await listen<string>('assistant-reply-turn', (event) => {
                setMessages(prev => {
                    const last = prev[prev.length - 1];
                    if (last && last.id === -1) {
                        const updated = [...prev];
                        updated[updated.length - 1] = { ...last, content: event.payload };
                        return updated;
                    }
                    return [...prev, { id: -1, role: 'assistant', content: event.payload, created_at: new Date().toISOString() }];
                });
            });

            unlistenClear = await listen('assistant-reply-clear', () => {
                setMessages(prev => prev.filter(m => m.id !== -1));
            });

            unlistenMsg = await listen<ChatMessage>('assistant-message', (event) => {
                setMessages(prev => [...prev.filter(m => m.id !== event.payload.id), event.payload]);
            });

            unlistenToolStart = await listen<string[]>('tool-execution-start', (event) => {
                setIsThinking(true);
                setToolStatus(Array.isArray(event.payload) ? event.payload : []);
            });
            unlistenToolEnd = await listen('tool-execution-end', () => {
                setIsThinking(false);
                setToolStatus([]);
            });
        }

        setup();
        return () => {
            unlistenTurn?.();
            unlistenClear?.();
            unlistenMsg?.();
            unlistenToolStart?.();
            unlistenToolEnd?.();
        };
    }, []);

    useEffect(() => {
        if (messages.length === 0) return;
        if (isFirstLoad.current) {
            scrollToBottom(true);
            isFirstLoad.current = false;
        } else {
            const last = messages[messages.length - 1];
            scrollToBottom(last?.id === null || last?.id === -1);
        }
    }, [messages]);

    const adjustInputHeight = () => {
        if (inputRef.current) {
            inputRef.current.style.height = 'auto';
            inputRef.current.style.height = `${Math.min(inputRef.current.scrollHeight, 140)}px`;
        }
    };
    useEffect(() => { adjustInputHeight(); }, [inputValue]);

    async function sendMessage(text: string) {
        const userMessage = text.trim();
        if (!userMessage || isLoading) return;

        setInputValue('');
        setError(null);
        setIsLoading(true);

        const tempMessage: ChatMessage = {
            id: null, role: 'user', content: userMessage, created_at: new Date().toISOString(),
        };
        setMessages(prev => [...prev, tempMessage]);

        try {
            const response = await invoke<SendMessageResponse>('send_chat_message', {
                request: { message: userMessage, session_id: null, base64_image: null },
            });
            setMessages(prev => {
                const filtered = prev.filter(m => m.id !== null && m.id !== -1);
                return [...filtered, response.user_message, response.assistant_message];
            });
        } catch (err) {
            setError(String(err));
            setMessages(prev => prev.filter(m => m.id !== null && m.id !== -1));
        } finally {
            setIsLoading(false);
            setIsThinking(false);
            setToolStatus([]);
        }
    }

    function handleKeyDown(event: React.KeyboardEvent<HTMLTextAreaElement>) {
        if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault();
            sendMessage(inputValue);
        }
    }

    const isEmpty = messages.length === 0 && !isLoading;

    return (
        <div className="relative flex flex-1 min-h-0 w-full flex-col">
            {/* Message stream */}
            <div
                ref={scrollRef}
                onScroll={handleScroll}
                className="chat-messages flex-1 min-h-0 overflow-y-auto"
            >
                <div className="mx-auto flex w-full max-w-3xl flex-col gap-1 px-1">
                    {isEmpty && (
                        <div className="flex flex-col items-center justify-center gap-2 py-20 text-center text-foreground-secondary">
                            <img src="/logo.png" alt="Lumen" className="mb-2 h-14 w-14 opacity-90" />
                            <p className="text-lg font-semibold text-foreground">Hi! I'm Lumen. 🐈</p>
                            <p className="text-sm">Ask me anything — this is the full-size chat.</p>
                            <div className="mt-5 grid w-full max-w-md grid-cols-2 gap-2">
                                {QUICK_ACTIONS.map((action, i) => (
                                    <button
                                        key={i}
                                        onClick={() => sendMessage(action)}
                                        disabled={isLoading}
                                        className="rounded-lg border border-border-light bg-background-secondary px-3 py-2.5 text-left text-xs text-foreground-secondary transition-colors hover:border-accent hover:bg-accent-light hover:text-accent disabled:opacity-50"
                                    >
                                        {action}
                                    </button>
                                ))}
                            </div>
                        </div>
                    )}

                    {messages.map((message, index) => (
                        <MessageBubble
                            key={message.id || index}
                            message={message}
                            assistantWidth="max-w-[64ch]"
                            userWidth="max-w-[80%]"
                        />
                    ))}

                    {isLoading && !messages.some(m => m.id === -1) && (
                        <ThinkingBubble isThinking={isThinking} toolStatus={toolStatus} />
                    )}

                    {error && (
                        <div className="my-2 rounded-md border border-border-light bg-background-secondary px-3 py-2 text-xs text-error">
                            {error}
                        </div>
                    )}

                    <div className="h-2" />
                    <div ref={messagesEndRef} />
                </div>
            </div>

            {/* Jump-to-bottom pill */}
            {showScrollDown && (
                <button
                    onClick={() => scrollToBottom()}
                    title="Jump to latest"
                    className="absolute bottom-24 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1.5 rounded-full border border-border bg-elevated px-3 py-1.5 text-xs font-medium text-foreground-secondary shadow-md transition-transform hover:-translate-x-1/2 hover:scale-105"
                >
                    <ArrowDown size={13} />
                    Latest
                </button>
            )}

            {/* Composer */}
            <div className="shrink-0 pt-3">
                <div className="mx-auto w-full max-w-3xl">
                    <div className="chat-input-container">
                        <textarea
                            ref={inputRef}
                            className="chat-input"
                            placeholder="Ask anything…"
                            value={inputValue}
                            onChange={(e) => setInputValue(e.target.value)}
                            onKeyDown={handleKeyDown}
                            rows={1}
                            disabled={isLoading}
                        />
                        <button
                            className={`chat-send-btn${isLoading ? ' is-stop' : ''}`}
                            onClick={isLoading
                                ? () => { invoke('cancel_chat').catch(err => console.error('cancel_chat failed:', err)); }
                                : () => sendMessage(inputValue)}
                            disabled={!isLoading && !inputValue.trim()}
                            title={isLoading ? 'Stop generating' : 'Send'}
                        >
                            {isLoading ? <Square size={14} fill="currentColor" /> : <Send size={16} />}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}

export default CatView;
