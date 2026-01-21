//INFO: Overlay Window - Chat panel
//NOTE: Clean minimal chat interface

import { useState, useEffect, useRef } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Send, X, Loader2, FileText, Scan, CalendarDays, LayoutDashboard } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

//INFO: Chat message type
interface ChatMessage {
    id: number | null;
    role: 'user' | 'assistant';
    content: string;
    created_at: string;
    image_data?: string;
}

interface SendMessageResponse {
    user_message: ChatMessage;
    assistant_message: ChatMessage;
}

function OverlayWindow() {
    const [messages, setMessages] = useState<ChatMessage[]>([]);
    const [inputValue, setInputValue] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [isCapturing, setIsCapturing] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [capturedImage, setCapturedImage] = useState<string | null>(null);
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLTextAreaElement>(null);

    //INFO: Set transparent background for overlay window
    useEffect(() => {
        document.body.classList.add('overlay-window');
        document.documentElement.classList.add('overlay-window');

        //INFO: Focus and resize input when window is mounted
        requestAnimationFrame(() => {
            adjustInputHeight();
            inputRef.current?.focus();
        });
        // Second pass after a short delay to catch any late layout shifts
        const timer = setTimeout(adjustInputHeight, 100);

        return () => {
            document.body.classList.remove('overlay-window');
            document.documentElement.classList.remove('overlay-window');
            clearTimeout(timer);
        };
    }, []);

    const scrollToBottom = (instant = false) => {
        // Use a small timeout to ensure layout is complete
        setTimeout(() => {
            messagesEndRef.current?.scrollIntoView({
                behavior: instant ? 'instant' : 'smooth',
                block: 'end'
            });
        }, 50);
    };

    //INFO: Listen for streaming "double-texting" messages and proactive updates
    useEffect(() => {
        let unlistenTurn: (() => void) | null = null;
        let unlistenMsg: (() => void) | null = null;

        async function setup() {
            // @ts-ignore
            const { listen } = await import('@tauri-apps/api/event');

            // Turn-by-turn streaming (Temporary)
            unlistenTurn = await listen<string>('assistant-reply-turn', (event) => {
                setMessages(prev => {
                    // Check if the last message is already a turn and has the same content (deduplicate)
                    const last = prev[prev.length - 1];
                    if (last?.id === -1 && last.content === event.payload) return prev;

                    const newPart: ChatMessage = {
                        id: -1,
                        role: 'assistant',
                        content: event.payload,
                        created_at: new Date().toISOString()
                    };
                    return [...prev, newPart];
                });
            });

            // Permanent proactive messages (from Agent)
            unlistenMsg = await listen<ChatMessage>('assistant-message', (event) => {
                setMessages(prev => [...prev.filter(m => m.id !== event.payload.id), event.payload]);
            });
        }
        setup();
        return () => {
            if (unlistenTurn) unlistenTurn();
            if (unlistenMsg) unlistenMsg();
        };
    }, []);

    //INFO: Listen for window focus events
    useEffect(() => {
        let unlisten: (() => void) | null = null;

        async function setupListener() {
            // @ts-ignore
            const { listen } = await import('@tauri-apps/api/event');
            unlisten = await listen('tauri://focus', () => {
                inputRef.current?.focus();
                adjustInputHeight();
                // When window "comes up", ensure we are at the bottom
                scrollToBottom(true);
            });
        }

        setupListener();
        return () => {
            if (unlisten) unlisten();
        };
    }, []);

    useEffect(() => {
        loadChatHistory();
    }, []);

    const isFirstLoad = useRef(true);

    // Scroll to bottom when messages change
    useEffect(() => {
        if (messages.length > 0) {
            if (isFirstLoad.current) {
                scrollToBottom(true);
                isFirstLoad.current = false;
            } else {
                const lastMessage = messages[messages.length - 1];
                // Smooth scroll for new messages (id is null for temp user msg, or -1 for turns)
                scrollToBottom(lastMessage?.id === null || lastMessage?.id === -1);
            }
        }
    }, [messages]);

    //INFO: Auto-resize textarea logic
    const adjustInputHeight = () => {
        if (inputRef.current) {
            inputRef.current.style.height = 'auto';
            const newHeight = Math.min(inputRef.current.scrollHeight, 100);
            inputRef.current.style.height = `${newHeight}px`;
        }
    };

    useEffect(() => {
        adjustInputHeight();
    }, [inputValue]);

    async function loadChatHistory() {
        try {
            const history = await invoke<ChatMessage[]>('get_chat_history', { sessionId: null, limit: 50 });
            setMessages(history);
        } catch (err) {
            console.error('Failed to load chat history:', err);
        }
    }

    //INFO: Listen for snipped images from the snipper window
    useEffect(() => {
        let unlisten: (() => void) | null = null;
        async function setupSnipListener() {
            // @ts-ignore
            const { listen } = await import('@tauri-apps/api/event');
            unlisten = await listen('snipped-image', (event: any) => {
                setCapturedImage(event.payload);
                // Ensure overlay is visible (should be handled by backend but double check)
            });
        }
        setupSnipListener();
        return () => {
            if (unlisten) unlisten();
        };
    }, []);

    async function handleCaptureScreen() {
        try {
            await invoke('start_snipping');
        } catch (err) {
            console.error('Failed to start snipping:', err);
        }
    }

    async function handleSendMessage() {
        if (!inputValue.trim() || isLoading) return;

        const userMessage = inputValue.trim();
        const base64Image = capturedImage;
        setInputValue('');
        setCapturedImage(null);
        setError(null);
        setIsLoading(true);

        //INFO: Add temporary user message
        const tempMessage: ChatMessage = {
            id: null,
            role: 'user',
            content: userMessage,
            created_at: new Date().toISOString(),
            image_data: base64Image || undefined
        };
        setMessages(prev => [...prev, tempMessage]);

        try {
            const response = await invoke<SendMessageResponse>('send_chat_message', {
                request: {
                    message: userMessage,
                    session_id: null,
                    base64_image: base64Image
                }
            });

            setMessages(prev => {
                // Filter out the temp user message (id: null) 
                // and any streamed turns (id: -1) from this interaction
                const filtered = prev.filter(m => m.id !== null && m.id !== -1);
                return [
                    ...filtered,
                    response.user_message,
                    response.assistant_message
                ];
            });
        } catch (err) {
            setError(String(err));
            setMessages(prev => prev.filter(m => m.id !== null && m.id !== -1));
        } finally {
            setIsLoading(false);
        }
    }

    function handleKeyDown(event: React.KeyboardEvent<HTMLTextAreaElement>) {
        if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault();
            handleSendMessage();
        }
        if (event.key === 'Escape') {
            hideOverlay();
        }
    }

    async function hideOverlay() {
        try {
            await invoke('hide_overlay');
        } catch (err) {
            console.error('Failed to hide overlay:', err);
        }
    }

    return (
        <div className="overlay-container">
            <div className="overlay-panel">
                {/* Messages */}
                <div className="overlay-content">
                    <div className="chat-messages">
                        {messages.length === 0 && !isLoading && (
                            <div className="welcome-message">
                                <img src="/logo.png" alt="Lumen Logo" style={{ width: '48px', height: '48px', marginBottom: 'var(--spacing-3)', opacity: 0.8 }} />
                                <p>Hi! I'm Lumen.</p>
                                <p style={{ fontSize: 'var(--font-size-sm)' }}>Ask me anything.</p>
                            </div>
                        )}

                        {messages.map((message, index) => (
                            <div key={message.id || index} className={`chat-message ${message.role}`}>
                                {message.image_data && (
                                    <div className="chat-message-image" style={{ marginBottom: 'var(--spacing-2)' }}>
                                        <img
                                            src={`data:image/png;base64,${message.image_data}`}
                                            alt="Observation"
                                            style={{
                                                maxWidth: '100%',
                                                maxHeight: '200px',
                                                borderRadius: 'var(--radius-md)',
                                                border: '1px solid rgba(0,0,0,0.1)'
                                            }}
                                        />
                                    </div>
                                )}
                                <div className="markdown-content">
                                    <ReactMarkdown
                                        remarkPlugins={[remarkGfm]}
                                        components={{
                                            code: ({ node, ...props }: any) => {
                                                const { inline, ...rest } = props;
                                                return (
                                                    <code
                                                        className={inline ? 'inline-code' : 'block-code'}
                                                        {...rest}
                                                    />
                                                );
                                            },
                                            a: ({ node, ...props }) => {
                                                const href = props.href || '';
                                                if (href.startsWith('lumen://open')) {
                                                    return (
                                                        <a
                                                            {...props}
                                                            href="#"
                                                            onClick={(e) => {
                                                                e.preventDefault();
                                                                try {
                                                                    const url = new URL(href);
                                                                    const rawPath = url.searchParams.get('path');
                                                                    if (rawPath) {
                                                                        const path = decodeURIComponent(rawPath);
                                                                        invoke('open_path', { path });
                                                                    }
                                                                } catch (err) {
                                                                    console.error('Failed to parse lumen link', err);
                                                                }
                                                            }}
                                                            className="lumen-pill"
                                                        >
                                                            <span className="lumen-pill-icon">
                                                                <FileText size={12} />
                                                            </span>
                                                            {props.children}
                                                        </a>
                                                    );
                                                }
                                                return <a {...props} target="_blank" rel="noopener noreferrer" />;
                                            }
                                        }}
                                    >
                                        {message.content}
                                    </ReactMarkdown>
                                </div>
                            </div>
                        ))}

                        {isLoading && (
                            <div className="chat-message assistant">
                                <div className="typing-indicator">
                                    <div className="typing-dot"></div>
                                    <div className="typing-dot"></div>
                                    <div className="typing-dot"></div>
                                </div>
                            </div>
                        )}

                        {error && <div className="error-message">{error}</div>}

                        <div ref={messagesEndRef} />
                    </div>
                </div>

                {/* Floating Action Bar */}
                <div className="floating-action-bar">
                    <button
                        className={`action-button camera-btn ${isCapturing ? 'loading' : ''}`}
                        onClick={handleCaptureScreen}
                        disabled={isLoading || isCapturing}
                        title="Capture screen"
                    >
                        {isCapturing ? <Loader2 size={18} className="loading-spinner" /> : <Scan size={18} />}
                    </button>

                    <button className="action-button calendar-btn">
                        <CalendarDays size={16} />
                        <span>Calendar</span>
                    </button>

                    <button
                        className="action-button home-btn"
                        onClick={() => invoke('show_main_window')}
                        title="Go to Home"
                    >
                        <LayoutDashboard size={18} />
                    </button>
                </div>

                {/* Input */}
                <div className="overlay-footer">
                    {capturedImage && (
                        <div className="image-preview-container" style={{
                            marginBottom: 'var(--spacing-3)',
                            position: 'relative',
                            width: 'fit-content'
                        }}>
                            <img
                                src={`data:image/png;base64,${capturedImage}`}
                                alt="Captured"
                                style={{
                                    maxHeight: '120px',
                                    borderRadius: 'var(--radius-md)',
                                    border: '1px solid var(--color-border)'
                                }}
                            />
                            <button
                                className="btn btn-icon"
                                onClick={() => setCapturedImage(null)}
                                style={{
                                    position: 'absolute',
                                    top: '-8px',
                                    right: '-8px',
                                    width: '24px',
                                    height: '24px',
                                    background: 'var(--color-error)',
                                    color: 'white',
                                    border: 'none',
                                    borderRadius: '50%'
                                }}
                            >
                                <X size={12} />
                            </button>
                        </div>
                    )}

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
                            className="chat-send-btn"
                            onClick={handleSendMessage}
                            disabled={(!inputValue.trim() && !capturedImage) || isLoading}
                        >
                            {isLoading ? <Loader2 size={16} className="loading-spinner" /> : <Send size={16} />}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}

export default OverlayWindow;
