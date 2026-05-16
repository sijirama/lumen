//INFO: Overlay Window - Chat panel
//NOTE: Clean minimal chat interface

import { useState, useEffect, useRef } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Send, Square, X, Loader2, FileText, Crosshair, CalendarDays, LayoutDashboard, MessageSquare, CheckSquare, Maximize2, Minimize2, ChevronDown, Wrench, Mail, Camera, Globe, Bell, Brain, Clipboard, Cloud, CheckCircle2, AlertCircle } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import CalendarView from '../components/CalendarView';

//INFO: Quick action prompts shown on empty chat
const QUICK_ACTIONS = [
    "What's on my calendar today?",
    "Check my unread emails",
    "What did we discuss last time?",
    "Open my daily note",
];

//INFO: One tool call's audit trail — what Lumen ran, with what args, and what came back.
interface ToolInvocation {
    name: string;
    args: unknown;
    result: unknown;
    duration_ms: number;
    started_at: string;
    succeeded: boolean;
}

//INFO: Chat message type
interface ChatMessage {
    id: number | null;
    role: 'user' | 'assistant';
    content: string;
    created_at: string;
    image_data?: string;
    citations?: { title: string; url: string }[];
    tool_invocations?: ToolInvocation[];
}

//INFO: Response type for send_chat_message command
interface SendMessageResponse {
    user_message: ChatMessage;
    assistant_message: ChatMessage;
    suggested_date?: string | null;
    suggested_view?: string | null;
}

//INFO: Helper to extract domain from URL
const extractDomain = (url: string) => {
    try {
        const domain = new URL(url).hostname;
        return domain.replace(/^www\./, '');
    } catch {
        return 'source';
    }
};

//INFO: Premium Citation Stack component
function CitationStack({ citations }: { citations: { title: string; url: string }[] }) {
    const [isExpanded, setIsExpanded] = useState(false);

    return (
        <div className="citation-container">
            <div
                className="citation-stack-trigger"
                onClick={() => setIsExpanded(!isExpanded)}
                title={isExpanded ? "Collapse citations" : "View sources"}
            >
                <div className="citation-avatar-stack">
                    {citations.slice(0, 3).map((cite, i) => (
                        <div key={i} className="citation-avatar" style={{ zIndex: 10 - i }}>
                            <img
                                src={`https://www.google.com/s2/favicons?domain=${extractDomain(cite.url)}&sz=64`}
                                alt=""
                                onError={(e) => (e.currentTarget.src = 'https://www.google.com/s2/favicons?domain=google.com&sz=64')}
                            />
                        </div>
                    ))}
                    {citations.length > 3 && (
                        <div className="citation-avatar extra-count" style={{ zIndex: 0 }}>
                            <span style={{ fontSize: '10px', color: '#fff' }}>+{citations.length - 3}</span>
                        </div>
                    )}
                </div>
                <span className="citation-count-label">
                    {citations.length} {citations.length === 1 ? 'Source' : 'Sources'}
                </span>
            </div>

            {isExpanded && (
                <div className="citation-expanded-list">
                    {citations.map((cite, i) => (
                        <a
                            key={i}
                            href={cite.url}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="citation-item"
                        >
                            <div className="citation-item-icon">
                                <img
                                    src={`https://www.google.com/s2/favicons?domain=${extractDomain(cite.url)}&sz=64`}
                                    alt=""
                                />
                            </div>
                            <div className="citation-info">
                                <span className="citation-title">{cite.title}</span>
                                <span className="citation-domain">{extractDomain(cite.url)}</span>
                            </div>
                        </a>
                    ))}
                </div>
            )}
        </div>
    );
}

//INFO: Per-tool icon lookup — keeps the trace visually scannable.
const TOOL_ICON_MAP: Record<string, LucideIcon> = {
    get_unread_emails: Mail,
    send_email: Mail,
    take_screenshot: Camera,
    search_web: Globe,
    set_reminder: Bell,
    retrieve_past_memories: Brain,
    search_clipboard: Clipboard,
    get_weather: Cloud,
    get_google_calendar_events: CalendarDays,
    create_calendar_event: CalendarDays,
    delete_calendar_event: CalendarDays,
    list_google_tasks: CheckSquare,
    create_google_task: CheckSquare,
    read_file: FileText,
    write_file: FileText,
    read_file_lines: FileText,
    list_files: FileText,
    search_notes: FileText,
    search_filesystem: FileText,
    grep_file: FileText,
    edit_file_line: FileText,
    insert_at_line: FileText,
    delete_file_line: FileText,
    get_file_metadata: FileText,
    get_obsidian_vault_info: FileText,
};

function formatDuration(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(2)}s`;
}

function prettyJson(value: unknown): string {
    try {
        return JSON.stringify(value, null, 2);
    } catch {
        return String(value);
    }
}

//INFO: Pull a base64 PNG out of a tool result so it can be previewed inline.
//      Only screenshots set image_data — everything else returns null.
function getInlineImage(inv: ToolInvocation): string | null {
    if (inv.name !== 'take_screenshot') return null;
    const r = inv.result as { image_data?: unknown } | null | undefined;
    return r && typeof r.image_data === 'string' && r.image_data.length > 0 ? r.image_data : null;
}

//INFO: One row in the tool trace — click to expand args + result.
function ToolInvocationRow({ inv }: { inv: ToolInvocation }) {
    const [isOpen, setIsOpen] = useState(false);
    const Icon = TOOL_ICON_MAP[inv.name] ?? Wrench;
    const StatusIcon = inv.succeeded ? CheckCircle2 : AlertCircle;
    const inlineImage = getInlineImage(inv);

    // For screenshot trace rows, the b64 in `result` is huge and useless to
    // print — strip it out before stringifying for the JSON view.
    const displayResult = inlineImage
        ? { ...(inv.result as Record<string, unknown>), image_data: '[png, shown above]' }
        : inv.result;

    return (
        <div className={`tool-row ${inv.succeeded ? 'ok' : 'err'} ${isOpen ? 'open' : ''}`}>
            <button
                type="button"
                className="tool-row-header"
                onClick={() => setIsOpen(o => !o)}
                title={isOpen ? 'Collapse details' : 'Show args + result'}
            >
                <Icon size={13} className="tool-row-icon" />
                <span className="tool-row-name">{inv.name}</span>
                <StatusIcon size={11} className="tool-row-status" />
                <span className="tool-row-duration">{formatDuration(inv.duration_ms)}</span>
                <ChevronDown size={12} className="tool-row-chevron" />
            </button>
            {isOpen && (
                <div className="tool-row-body">
                    {inlineImage && (
                        <div className="tool-row-section">
                            <div className="tool-row-label">Capture</div>
                            <img
                                src={`data:image/png;base64,${inlineImage}`}
                                alt="Screenshot"
                                className="tool-row-thumb"
                            />
                        </div>
                    )}
                    <div className="tool-row-section">
                        <div className="tool-row-label">Args</div>
                        <pre className="tool-row-pre">{prettyJson(inv.args)}</pre>
                    </div>
                    <div className="tool-row-section">
                        <div className="tool-row-label">Result</div>
                        <pre className="tool-row-pre">{prettyJson(displayResult)}</pre>
                    </div>
                </div>
            )}
        </div>
    );
}

//INFO: The outer collapsible — "N tools used (Xms total)" with rows inside.
function ToolTrace({ invocations }: { invocations: ToolInvocation[] }) {
    const [isOpen, setIsOpen] = useState(false);
    const total = invocations.reduce((sum, inv) => sum + inv.duration_ms, 0);
    const anyFailed = invocations.some(inv => !inv.succeeded);

    return (
        <div className={`tool-trace ${isOpen ? 'open' : ''}`}>
            <button
                type="button"
                className="tool-trace-trigger"
                onClick={() => setIsOpen(o => !o)}
                title={isOpen ? 'Collapse tool trace' : 'Show tool trace'}
            >
                <Wrench size={12} className="tool-trace-icon" />
                <span className="tool-trace-label">
                    {invocations.length} {invocations.length === 1 ? 'tool' : 'tools'} used
                </span>
                <span className="tool-trace-total">{formatDuration(total)}</span>
                {anyFailed && <AlertCircle size={11} className="tool-trace-warn" />}
                <ChevronDown size={12} className="tool-trace-chevron" />
            </button>
            {isOpen && (
                <div className="tool-trace-list">
                    {invocations.map((inv, i) => (
                        <ToolInvocationRow key={i} inv={inv} />
                    ))}
                </div>
            )}
        </div>
    );
}

function OverlayWindow() {
    const [messages, setMessages] = useState<ChatMessage[]>([]);
    const [inputValue, setInputValue] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [isThinking, setIsThinking] = useState(false);
    const [isCapturing, setIsCapturing] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [capturedImage, setCapturedImage] = useState<string | null>(null);
    const [currentView, setCurrentView] = useState<'chat' | 'calendar'>('chat');
    const [transitionView, setTransitionView] = useState<'chat' | 'calendar'>('chat');
    const [isCalendarExpanded, setIsCalendarExpanded] = useState(false);
    const [suggestedDate, setSuggestedDate] = useState<string | undefined>(undefined);

    // Content size toggle
    const [contentLarge, setContentLarge] = useState(false);

    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLTextAreaElement>(null);

    //INFO: Orchestrate smooth view switching
    const switchView = async (newView: 'chat' | 'calendar') => {
        if (newView === transitionView) return;
        setTransitionView(newView);
        setCurrentView(newView);
        if (newView === 'chat') {
            setTimeout(() => scrollToBottom(true), 300);
        } else if (newView === 'calendar') {
            setIsCalendarExpanded(true);
        }
    };

    //INFO: Toggle calendar expansion (Pure CSS animation now)
    const handleCalendarExpansionToggle = (expanded: boolean) => {
        setIsCalendarExpanded(expanded);
    };

    //INFO: Set transparent background for overlay window
    useEffect(() => {
        document.body.classList.add('overlay-window');
        document.documentElement.classList.add('overlay-window');

        //INFO: Focus and resize input when window is mounted
        requestAnimationFrame(() => {
            adjustInputHeight();
            inputRef.current?.focus();
        });

        loadChatHistory();

        return () => {
            document.body.classList.remove('overlay-window');
            document.documentElement.classList.remove('overlay-window');
        };
    }, []);

    const scrollToBottom = (instant = false) => {
        // Use a small timeout for smooth scrolling to let layout settle,
        // but go instant for view switches/mounts
        const performScroll = () => {
            messagesEndRef.current?.scrollIntoView({
                behavior: instant ? 'instant' : 'smooth',
                block: 'end'
            });
        };

        if (instant) {
            requestAnimationFrame(performScroll);
        } else {
            setTimeout(performScroll, 50);
        }
    };

    //INFO: Listen for streaming messages, clear events, proactive updates, tasks, and morning briefing
    useEffect(() => {
        let unlistenTurn: (() => void) | null = null;
        let unlistenClear: (() => void) | null = null;
        let unlistenMsg: (() => void) | null = null;
        let unlistenToolStart: (() => void) | null = null;
        let unlistenToolEnd: (() => void) | null = null;
        let unlistenMorningBriefing: (() => void) | null = null;

        async function setup() {
            // @ts-ignore
            const { listen } = await import('@tauri-apps/api/event');

            // Real-time text from the AI (replaces streaming bubble content)
            unlistenTurn = await listen<string>('assistant-reply-turn', (event) => {
                setMessages(prev => {
                    const last = prev[prev.length - 1];
                    if (last && last.id === -1) {
                        // Replace the streaming bubble content (non-streaming sends full text)
                        const updated = [...prev];
                        updated[updated.length - 1] = {
                            ...last,
                            content: event.payload
                        };
                        return updated;
                    } else {
                        // Create a new streaming bubble
                        const newPart: ChatMessage = {
                            id: -1,
                            role: 'assistant',
                            content: event.payload,
                            created_at: new Date().toISOString()
                        };
                        return [...prev, newPart];
                    }
                });
            });

            // Clear streaming bubbles (e.g., during tool execution rounds)
            unlistenClear = await listen('assistant-reply-clear', () => {
                setMessages(prev => prev.filter(m => m.id !== -1));
            });

            // Permanent proactive messages (from Agent)
            unlistenMsg = await listen<ChatMessage>('assistant-message', (event) => {
                setMessages(prev => [...prev.filter(m => m.id !== event.payload.id), event.payload]);
            });

            // Tool execution state tracking
            unlistenToolStart = await listen('tool-execution-start', () => {
                setIsThinking(true);
            });
            unlistenToolEnd = await listen('tool-execution-end', () => {
                setIsThinking(false);
            });

            // Morning briefing proactive message
            unlistenMorningBriefing = await listen('morning-briefing-ready', () => {
                setMessages(prev => {
                    // Don't duplicate if id -2 already exists
                    if (prev.some(m => m.id === -2)) return prev;
                    const briefingMessage: ChatMessage = {
                        id: -2,
                        role: 'assistant',
                        content: "Good morning! Your daily briefing is ready. Ask me about your day or check the dashboard for details.",
                        created_at: new Date().toISOString()
                    };
                    return [...prev, briefingMessage];
                });
            });
        }

        setup();

        return () => {
            if (unlistenTurn) unlistenTurn();
            if (unlistenClear) unlistenClear();
            if (unlistenMsg) unlistenMsg();
            if (unlistenToolStart) unlistenToolStart();
            if (unlistenToolEnd) unlistenToolEnd();
            if (unlistenMorningBriefing) unlistenMorningBriefing();
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
            scrollToBottom(true);
        } catch (err) {
            console.error('Failed to load chat history:', err);
            setError('Failed to load chat history. Try reopening the overlay.');
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
                switchView('chat');
            });
        }
        setupSnipListener();
        return () => {
            if (unlisten) unlisten();
        };
    }, [transitionView]);

    async function handleCaptureScreen() {
        setIsCapturing(true);
        try {
            await invoke('start_snipping');
        } catch (err) {
            console.error('Failed to start snipping:', err);
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

        if (transitionView !== 'chat') {
            switchView('chat');
        }

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

            //INFO: Trigger implicit view transition if suggested
            if (response.suggested_view) {
                if (response.suggested_date) {
                    setSuggestedDate(response.suggested_date);
                }

                if (response.suggested_view !== transitionView && response.suggested_view !== currentView) {
                    console.log(`🧠 Lumen suggested view transition: ${response.suggested_view}`);
                    switchView(response.suggested_view as 'chat' | 'calendar');
                }
            }
        } catch (err) {
            setError(String(err));
            setMessages(prev => prev.filter(m => m.id !== null && m.id !== -1));
        } finally {
            setIsLoading(false);
            setIsThinking(false);
        }
    }

    //INFO: Handle quick action chip click — set input and immediately send
    async function handleQuickAction(action: string) {
        if (isLoading) return;
        setInputValue(action);
        // Use a microtask to let state settle before sending
        await new Promise<void>(resolve => setTimeout(resolve, 0));
        // We bypass the state-based send and directly invoke with the known value
        const userMessage = action;
        const base64Image = capturedImage;
        setInputValue('');
        setCapturedImage(null);
        setError(null);
        setIsLoading(true);

        const tempMessage: ChatMessage = {
            id: null,
            role: 'user',
            content: userMessage,
            created_at: new Date().toISOString(),
            image_data: base64Image || undefined
        };
        setMessages(prev => [...prev, tempMessage]);

        if (transitionView !== 'chat') {
            switchView('chat');
        }

        try {
            const response = await invoke<SendMessageResponse>('send_chat_message', {
                request: {
                    message: userMessage,
                    session_id: null,
                    base64_image: base64Image
                }
            });

            setMessages(prev => {
                const filtered = prev.filter(m => m.id !== null && m.id !== -1);
                return [
                    ...filtered,
                    response.user_message,
                    response.assistant_message
                ];
            });

            if (response.suggested_view) {
                if (response.suggested_date) {
                    setSuggestedDate(response.suggested_date);
                }
                if (response.suggested_view !== transitionView && response.suggested_view !== currentView) {
                    switchView(response.suggested_view as 'chat' | 'calendar');
                }
            }
        } catch (err) {
            setError(String(err));
            setMessages(prev => prev.filter(m => m.id !== null && m.id !== -1));
        } finally {
            setIsLoading(false);
            setIsThinking(false);
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

                {/* Messages / Calendar */}
                <div className={`overlay-content${transitionView === 'calendar' && isCalendarExpanded ? ' expanded' : ''}${contentLarge && transitionView === 'chat' ? ' large' : ''}`}>

                    {/* ── Header row (inside the card) ── */}
                    <div className="overlay-chat-header">
                        {/* Left: status dot + wordmark */}
                        <div style={{ display: 'flex', alignItems: 'center', gap: '7px' }}>
                            <span className={`overlay-status-dot${isLoading || isThinking ? ' active' : ''}`} />
                            <span className="overlay-wordmark">Lumen</span>
                        </div>

                        {/* Right: resize */}
                        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                            <button
                                onClick={() => setContentLarge(v => !v)}
                                className="overlay-header-icon-btn"
                                title={contentLarge ? 'Shrink' : 'Expand'}
                            >
                                {contentLarge ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
                            </button>
                        </div>
                    </div>

                    <div className="view-transition-wrapper">
                        {/* Chat View — always rendered */}
                        <div className={`view-pane ${transitionView === 'chat' ? 'active' : ''} chat-messages`}>
                            {messages.length === 0 && !isLoading && (
                                <div className="welcome-message">
                                    <img src="/logo.png" alt="Lumen Logo" style={{ width: '48px', height: '48px', marginBottom: 'var(--spacing-3)', opacity: 0.8 }} />
                                    <p>Hi! I'm Lumen.</p>
                                    <p style={{ fontSize: 'var(--font-size-sm)' }}>Ask me anything.</p>
                                    {/* Quick action chips */}
                                    <div style={{
                                        display: 'grid',
                                        gridTemplateColumns: '1fr 1fr',
                                        gap: '8px',
                                        marginTop: '16px',
                                        width: '100%',
                                        maxWidth: '320px'
                                    }}>
                                        {QUICK_ACTIONS.map((action, i) => (
                                            <button
                                                key={i}
                                                className="quick-action-chip"
                                                onClick={() => handleQuickAction(action)}
                                            >
                                                {action}
                                            </button>
                                        ))}
                                    </div>
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
                                                pre: ({ children }: any) => (
                                                    <pre className="code-block-pre">{children}</pre>
                                                ),
                                                code: ({ node, className, children, ...props }: any) => {
                                                    // v10: no inline prop — detect via position spanning multiple lines
                                                    const isBlock = node?.position
                                                        ? node.position.end.line > node.position.start.line
                                                        : !!className;
                                                    const lang = (className || '').replace('language-', '');
                                                    if (isBlock) {
                                                        return (
                                                            <span className="block-code-wrapper">
                                                                {lang && <span className="code-lang-label">{lang}</span>}
                                                                <code className="block-code" {...props}>{children}</code>
                                                            </span>
                                                        );
                                                    }
                                                    return <code className="inline-code" {...props}>{children}</code>;
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
                                    {message.role === 'assistant' && (
                                        <div className="message-actions">
                                            <button
                                                className="msg-action-btn"
                                                title="Copy"
                                                onClick={() => navigator.clipboard.writeText(message.content)}
                                            >
                                                Copy
                                            </button>
                                        </div>
                                    )}
                                    {message.role === 'assistant' && message.tool_invocations && message.tool_invocations.length > 0 && (
                                        <ToolTrace invocations={message.tool_invocations} />
                                    )}
                                    {message.role === 'assistant' && message.citations && message.citations.length > 0 && (
                                        <CitationStack citations={message.citations} />
                                    )}
                                </div>
                            ))}

                            {isLoading && (
                                <div className="chat-message assistant">
                                    {isThinking ? (
                                        <div className="thinking-indicator">
                                            <div className="thinking-glyph">⚙</div>
                                            <span className="thinking-label">Working on it...</span>
                                        </div>
                                    ) : (
                                        <div className="typing-indicator">
                                            <div className="typing-dot"></div>
                                            <div className="typing-dot"></div>
                                            <div className="typing-dot"></div>
                                        </div>
                                    )}
                                </div>
                            )}

                            {error && <div className="error-message">{error}</div>}

                            <div className="chat-spacer" />
                            <div ref={messagesEndRef} />
                        </div>

                        {/* Calendar View */}
                        <div className={`view-pane ${transitionView === 'calendar' ? 'active' : ''} calendar-container`}>
                            <CalendarView
                                isExpanded={isCalendarExpanded}
                                onToggleExpand={handleCalendarExpansionToggle}
                                initialDate={suggestedDate}
                                transitionView={transitionView}
                            />
                        </div>

                    </div>
                </div>

                {/* Floating Action Bar */}
                <div className="floating-action-bar">
                    {/* 1. Camera */}
                    <button
                        className={`action-button camera-btn ${isCapturing ? 'loading' : ''}`}
                        onClick={handleCaptureScreen}
                        disabled={isLoading || isCapturing}
                        title="Capture screen"
                    >
                        {isCapturing ? <Loader2 size={18} className="loading-spinner" /> : <Crosshair size={18} />}
                    </button>

                    {/* 2. Calendar toggle */}
                    <button
                        className={`action-button ${transitionView === 'calendar' ? 'active' : ''} calendar-btn`}
                        onClick={() => switchView(transitionView === 'calendar' ? 'chat' : 'calendar')}
                    >
                        {transitionView === 'calendar' ? (
                            <>
                                <MessageSquare size={16} />
                                <span>Chat</span>
                            </>
                        ) : (
                            <>
                                <CalendarDays size={16} />
                                <span>Calendar</span>
                            </>
                        )}
                    </button>

                    {/* 4. Home */}
                    <button
                        className="action-button home-btn"
                        onClick={() => invoke('show_main_window')}
                        title="Go to Home"
                    >
                        <LayoutDashboard size={18} />
                    </button>
                </div>

                {/* Input Area (Persistent) */}
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
                                    maxWidth: '100%',
                                    maxHeight: '120px',
                                    borderRadius: 'var(--radius-md)',
                                    border: '1px solid var(--color-border)',
                                    objectFit: 'contain'
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
                            className={`chat-send-btn${isLoading ? ' is-stop' : ''}`}
                            onClick={isLoading
                                ? () => { invoke('cancel_chat').catch(err => console.error('cancel_chat failed:', err)); }
                                : handleSendMessage}
                            disabled={!isLoading && !inputValue.trim() && !capturedImage}
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

export default OverlayWindow;
