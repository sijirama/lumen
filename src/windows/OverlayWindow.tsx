//INFO: Overlay Window - Chat panel
//NOTE: Clean minimal chat interface

import { useState, useEffect, useRef } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Send, Sparkles, X, Loader2, Camera } from 'lucide-react';
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
    const [error, setError] = useState<string | null>(null);
    const [capturedImage, setCapturedImage] = useState<string | null>(null);
    const [isCapturing, setIsCapturing] = useState(false);
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLTextAreaElement>(null);

    //INFO: Set transparent background for overlay window
    useEffect(() => {
        document.body.classList.add('overlay-window');
        document.documentElement.classList.add('overlay-window');

        //INFO: Focus input when window is mounted
        inputRef.current?.focus();

        return () => {
            document.body.classList.remove('overlay-window');
            document.documentElement.classList.remove('overlay-window');
        };
    }, []);

    //INFO: Listen for window focus events to re-focus the input
    useEffect(() => {
        let unlisten: (() => void) | null = null;

        async function setupListener() {
            // @ts-ignore
            const { listen } = await import('@tauri-apps/api/event');
            unlisten = await listen('tauri://focus', () => {
                inputRef.current?.focus();
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

    // Scroll to bottom when messages change
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    // Scroll to bottom immediately when chat history loads
    useEffect(() => {
        if (messages.length > 0) {
            messagesEndRef.current?.scrollIntoView({ behavior: 'instant' });
        }
    }, [messages.length > 0]);

    //INFO: Auto-resize textarea
    useEffect(() => {
        if (inputRef.current) {
            inputRef.current.style.height = 'auto';
            const newHeight = Math.min(inputRef.current.scrollHeight, 100);
            inputRef.current.style.height = `${newHeight}px`;
        }
    }, [inputValue]);

    async function loadChatHistory() {
        try {
            const history = await invoke<ChatMessage[]>('get_chat_history', { sessionId: null, limit: 50 });
            setMessages(history);
        } catch (err) {
            console.error('Failed to load chat history:', err);
        }
    }

    async function handleCaptureScreen() {
        try {
            setIsCapturing(true);
            // Hide the overlay temporarily to capture the screen underneath
            await invoke('hide_overlay');
            // Small delay to ensure window is hidden
            await new Promise(r => setTimeout(r, 200));
            const b64 = await invoke<string>('capture_primary_screen');
            setCapturedImage(b64);
            await invoke('show_overlay');
        } catch (err) {
            console.error('Failed to capture screen:', err);
            await invoke('show_overlay');
        } finally {
            setIsCapturing(false);
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

            setMessages(prev => [
                ...prev.slice(0, -1),
                response.user_message,
                response.assistant_message
            ]);
        } catch (err) {
            setError(String(err));
            setMessages(prev => prev.slice(0, -1));
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
                {/* Header */}
                <div className="overlay-header">
                    <div className="overlay-title">
                        <Sparkles size={16} style={{ color: 'var(--color-accent)' }} />
                        <span>Lumen</span>
                    </div>
                    <button className="btn btn-ghost btn-icon" onClick={hideOverlay} style={{ width: '28px', height: '28px' }}>
                        <X size={14} />
                    </button>
                </div>

                {/* Messages */}
                <div className="overlay-content">
                    <div className="chat-messages">
                        {messages.length === 0 && !isLoading && (
                            <div className="welcome-message">
                                <Sparkles size={24} style={{ opacity: 0.3, marginBottom: 'var(--spacing-3)' }} />
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
                                            p: ({ node, ...props }) => <p style={{ margin: 0, marginBottom: index === messages.length - 1 ? 0 : '0.5em' }} {...props} />,
                                            ul: ({ node, ...props }) => <ul style={{ paddingLeft: '1.2em', margin: '0.5em 0' }} {...props} />,
                                            ol: ({ node, ...props }) => <ol style={{ paddingLeft: '1.2em', margin: '0.5em 0' }} {...props} />,
                                            code: ({ node, ...props }: any) => {
                                                const { inline, ...rest } = props;
                                                return (
                                                    <code
                                                        style={{
                                                            background: 'rgba(0,0,0,0.1)',
                                                            padding: '2px 4px',
                                                            borderRadius: '4px',
                                                            fontFamily: 'monospace',
                                                            fontSize: '0.9em'
                                                        }}
                                                        {...rest}
                                                    />
                                                );
                                            }
                                        }}
                                    >
                                        {message.content}
                                    </ReactMarkdown>
                                </div>
                            </div>
                        ))}

                        {isLoading && (
                            <div className="chat-message assistant" style={{ display: 'flex', alignItems: 'center', gap: 'var(--spacing-2)' }}>
                                <Loader2 size={14} className="loading-spinner" />
                                <span>Thinking...</span>
                            </div>
                        )}

                        {error && <div className="error-message">{error}</div>}

                        <div ref={messagesEndRef} />
                    </div>
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
                        <button
                            className={`btn btn-icon ${isCapturing ? 'loading' : ''}`}
                            onClick={handleCaptureScreen}
                            disabled={isLoading || isCapturing}
                            style={{
                                width: '36px',
                                height: '36px',
                                opacity: 0.6,
                                flexShrink: 0
                            }}
                            title="Lumen, look at my screen"
                        >
                            {isCapturing ? <Loader2 size={16} className="loading-spinner" /> : <Camera size={16} />}
                        </button>
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
