//INFO: ChatKit — the shared presentation layer for Lumen's chat, used by BOTH
//      the floating OverlayWindow and the full-screen "cat view" page in the
//      main window. There is ONE persistent chat (see feedback: single chat),
//      so both surfaces render identical bubbles/traces/citations from here —
//      no drift. The windows keep their own state + send/event wiring; this file
//      owns only the visuals.

import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
    FileText, Wrench, Mail, Camera, Globe, Bell, Brain, Clipboard, Cloud,
    CheckCircle2, AlertCircle, ChevronDown, CalendarDays,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

//INFO: One tool call's audit trail — what Lumen ran, with what args, and what came back.
export interface ToolInvocation {
    name: string;
    args: unknown;
    result: unknown;
    duration_ms: number;
    started_at: string;
    succeeded: boolean;
}

//INFO: Chat message type
export interface ChatMessage {
    id: number | null;
    role: 'user' | 'assistant';
    content: string;
    created_at: string;
    image_data?: string;
    citations?: { title: string; url: string }[];
    tool_invocations?: ToolInvocation[];
}

//INFO: Response type for send_chat_message command
export interface SendMessageResponse {
    user_message: ChatMessage;
    assistant_message: ChatMessage;
}

//INFO: Helper to extract domain from URL
export const extractDomain = (url: string) => {
    try {
        const domain = new URL(url).hostname;
        return domain.replace(/^www\./, '');
    } catch {
        return 'source';
    }
};

//INFO: Premium Citation Stack component
export function CitationStack({ citations }: { citations: { title: string; url: string }[] }) {
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
export const TOOL_ICON_MAP: Record<string, LucideIcon> = {
    get_unread_emails: Mail,
    send_email: Mail,
    take_screenshot: Camera,
    search_web: Globe,
    set_reminder: Bell,
    retrieve_past_memories: Brain,
    search_memories: Brain,
    remember_this: Brain,
    edit_memory: Brain,
    forget_memory: Brain,
    view_runtime_logs: Wrench,
    search_clipboard: Clipboard,
    get_weather: Cloud,
    get_google_calendar_events: CalendarDays,
    create_calendar_event: CalendarDays,
    delete_calendar_event: CalendarDays,
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
export function ToolTrace({ invocations }: { invocations: ToolInvocation[] }) {
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

//INFO: A single chat bubble (avatar + image + markdown + actions + trace + citations).
//      Shared between the overlay and the cat view so they never drift. Bubble
//      width is a Tailwind class passed in (the base `.chat-message` no longer
//      hard-codes it): the overlay keeps 85%, the wide cat view narrows the
//      assistant to a readable measure.
export function MessageBubble({
    message,
    userWidth = 'max-w-[85%]',
    assistantWidth = 'max-w-[85%]',
}: {
    message: ChatMessage;
    userWidth?: string;
    assistantWidth?: string;
}) {
    const widthClass = message.role === 'user' ? userWidth : assistantWidth;
    return (
        <div className={`flex w-full items-start gap-1 ${message.role === 'user' ? 'justify-end' : 'justify-start'}`}>
            {message.role === 'assistant' && (
                <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center" aria-hidden="true">
                    <img src="/logo.png" alt="" className="h-full w-full object-contain" />
                </div>
            )}
            <div className={`chat-message group ${message.role} ${widthClass}`}>
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
                    {message.id === -1 && (
                        <span className="stream-cursor" aria-hidden="true">▍</span>
                    )}
                </div>
                {message.role === 'assistant' && message.id !== -1 && (
                    <div className="mt-1 flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                        <button
                            className="cursor-pointer rounded-sm border border-border px-[7px] py-0.5 text-[11px] text-muted transition-colors hover:bg-background-secondary hover:text-foreground"
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
        </div>
    );
}

//INFO: The "Lumen is working/typing" bubble shown while a turn is in flight.
//      `toolStatus` carries humanized labels for the tools running this round.
export function ThinkingBubble({ isThinking, toolStatus }: { isThinking: boolean; toolStatus: string[] }) {
    return (
        <div className="flex w-full items-start justify-start gap-1">
            <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center" aria-hidden="true">
                <img src="/logo.png" alt="" className="h-full w-full object-contain" />
            </div>
            <div className="chat-message assistant">
                {isThinking ? (
                    <div className="thinking-indicator">
                        <div className="thinking-glyph">⚙</div>
                        <span className="thinking-label">
                            {toolStatus.length > 0 ? `${toolStatus.join(' · ')}…` : 'Working on it…'}
                        </span>
                    </div>
                ) : (
                    <div className="typing-indicator">
                        <div className="typing-dot"></div>
                        <div className="typing-dot"></div>
                        <div className="typing-dot"></div>
                    </div>
                )}
            </div>
        </div>
    );
}
