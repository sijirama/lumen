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
        <div className="cat-chat">
            <div
                ref={scrollRef}
                onScroll={handleScroll}
                className="cat-chat-scroll"
            >
                <div className="cat-chat-stream">
                    {isEmpty && (
                        <div className="cat-empty-state">
                            <img src="/logo.png" alt="Lumen" className="cat-empty-logo" />
                            <p className="cat-empty-title">Hi! I'm Lumen.</p>
                            <p className="cat-empty-subtitle">Ask me anything.</p>
                            <div className="cat-quick-actions">
                                {QUICK_ACTIONS.map((action, i) => (
                                    <button
                                        key={i}
                                        onClick={() => sendMessage(action)}
                                        disabled={isLoading}
                                        className="cat-quick-action"
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
                            assistantMaxWidth="72%"
                            userMaxWidth="46%"
                        />
                    ))}

                    {isLoading && !messages.some(m => m.id === -1) && (
                        <ThinkingBubble isThinking={isThinking} toolStatus={toolStatus} />
                    )}

                    {error && (
                        <div className="cat-chat-error">
                            {error}
                        </div>
                    )}

                    <div ref={messagesEndRef} />
                </div>
            </div>

            {showScrollDown && (
                <button
                    onClick={() => scrollToBottom()}
                    title="Jump to latest"
                    className="cat-scroll-latest"
                >
                    <ArrowDown size={13} />
                    Latest
                </button>
            )}

            <div className="cat-composer">
                <div className="chat-input-container">
                    <textarea
                        ref={inputRef}
                        className="chat-input"
                        placeholder="Ask anything..."
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
                        {isLoading ? <Square size={13} fill="currentColor" /> : <Send size={15} />}
                    </button>
                </div>
            </div>
        </div>
    );
}

export default CatView;
