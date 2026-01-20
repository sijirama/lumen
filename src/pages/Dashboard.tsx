import { useState, useEffect } from 'react';
import { RefreshCw, Volume2, VolumeX } from 'lucide-react';
import { format } from 'date-fns';
import { invoke } from '@tauri-apps/api/core';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

//INFO: Props interface
interface DashboardProps {
    userName: string;
}

interface Briefing {
    content: string;
    created_at: string;
    is_stale: boolean;
}

function Dashboard({ userName }: DashboardProps) {
    //INFO: State
    const [currentTime, setCurrentTime] = useState(new Date());
    const [briefing, setBriefing] = useState<Briefing | null>(null);
    const [loading, setLoading] = useState(false);
    const [refreshing, setRefreshing] = useState(false);
    const [isSpeaking, setIsSpeaking] = useState(false);
    const [voices, setVoices] = useState<SpeechSynthesisVoice[]>([]);

    // Load voices on mount (required for Web Speech API)
    useEffect(() => {
        // Safety check - speechSynthesis may not be available in all environments
        if (typeof window === 'undefined' || !window.speechSynthesis) {
            return;
        }

        const loadVoices = () => {
            const availableVoices = window.speechSynthesis.getVoices();
            if (availableVoices.length > 0) {
                setVoices(availableVoices);
            }
        };

        loadVoices();
        window.speechSynthesis.onvoiceschanged = loadVoices;

        return () => {
            if (window.speechSynthesis) {
                window.speechSynthesis.onvoiceschanged = null;
            }
        };
    }, []);

    const handleSpeak = () => {
        // Safety check
        if (!window.speechSynthesis) {
            console.warn('Speech synthesis not available');
            return;
        }

        if (isSpeaking) {
            window.speechSynthesis.cancel();
            setIsSpeaking(false);
            return;
        }

        if (!briefing) return;

        // Strip markdown links for cleaner speech
        const cleanText = briefing.content
            .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
            .replace(/[*_#`]/g, '');

        const utterance = new SpeechSynthesisUtterance(cleanText);
        utterance.rate = 0.9;
        utterance.pitch = 1.0;

        // Try to find a nice neural-sounding voice
        const preferredVoice = voices.find(v =>
            v.name.includes('Google') || v.name.includes('Neural') || v.name.includes('English')
        ) || voices[0];

        if (preferredVoice) utterance.voice = preferredVoice;

        utterance.onend = () => setIsSpeaking(false);
        utterance.onerror = () => setIsSpeaking(false);
        setIsSpeaking(true);
        window.speechSynthesis.speak(utterance);
    };

    //INFO: Update time every minute
    useEffect(() => {
        const interval = setInterval(() => {
            setCurrentTime(new Date());
        }, 60000);
        return () => clearInterval(interval);
    }, []);

    //INFO: Fetch initial briefing on mount
    useEffect(() => {
        loadBriefing();
    }, []);

    async function loadBriefing() {
        try {
            setLoading(true);
            const result = await invoke<Briefing | null>('get_dashboard_briefing');
            setBriefing(result);

            // If it's stale or missing, auto-refresh in background (if user name is set)
            if (!result || result.is_stale) {
                refreshBriefing();
            }
        } catch (err) {
            console.error('Failed to load briefing:', err);
        } finally {
            setLoading(false);
        }
    }

    async function refreshBriefing() {
        if (refreshing) return;
        try {
            setRefreshing(true);
            const result = await invoke<Briefing>('refresh_dashboard_briefing');
            setBriefing(result);
        } catch (err) {
            console.error('Failed to refresh briefing:', err);
        } finally {
            setRefreshing(false);
        }
    }

    //INFO: Get time-appropriate greeting
    function getGreeting(): string {
        const hour = currentTime.getHours();
        if (hour >= 5 && hour < 12) return 'Good morning';
        if (hour >= 12 && hour < 17) return 'Good afternoon';
        if (hour >= 17 && hour < 21) return 'Good evening';
        return 'Hey';
    }

    return (
        <div className="animate-fade-in" style={{ paddingBottom: 'var(--spacing-12)' }}>
            <div className="greeting">
                <div className="greeting-info">
                    <h1 className="greeting-text">
                        {getGreeting()}, {userName}
                    </h1>
                    <p className="greeting-date">
                        {format(currentTime, "EEEE, MMMM d")}
                    </p>
                </div>
                <div className="briefing-actions">
                    <button
                        className={`btn btn-ghost btn-icon ${isSpeaking ? 'active' : ''}`}
                        onClick={handleSpeak}
                        title={isSpeaking ? "Stop Speaking" : "Listen to Briefing"}
                        style={{ color: isSpeaking ? 'var(--color-accent)' : 'inherit' }}
                    >
                        {isSpeaking ? <VolumeX size={18} /> : <Volume2 size={18} />}
                    </button>
                    <button
                        className={`btn btn-ghost btn-icon ${refreshing ? 'loading' : ''}`}
                        onClick={refreshBriefing}
                        disabled={refreshing}
                        title="Refresh Briefing"
                    >
                        <RefreshCw size={18} className={refreshing ? 'loading-spinner' : ''} />
                    </button>
                </div>
            </div>

            {/* Briefing Card */}
            <div className={`briefing-card ${(loading || refreshing) ? 'loading' : ''}`}>
                <div className="shimmer-overlay"></div>
                <div className="briefing-header">
                    <div className="briefing-title">
                        <span role="img" aria-label="sparkles">✨</span> Daily Briefing
                    </div>
                    <div className="briefing-status">
                        {briefing?.is_stale && !refreshing && <span className="stale-badge">Updates available</span>}
                        {briefing && !refreshing && (
                            <span>Last updated {format(new Date(briefing.created_at), 'h:mm a')}</span>
                        )}
                    </div>
                </div>

                <div className="briefing-content">
                    {briefing ? (
                        <ReactMarkdown
                            remarkPlugins={[remarkGfm]}
                            components={{
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
                                                        const path = url.searchParams.get('path');
                                                        if (path) {
                                                            invoke('open_path', { path });
                                                        }
                                                    } catch (err) {
                                                        console.error('Failed to parse lumen link', err);
                                                    }
                                                }}
                                                className="lumen-link"
                                            >
                                                {props.children}
                                            </a>
                                        );
                                    }
                                    return <a {...props} target="_blank" rel="noopener noreferrer" />;
                                }
                            }}
                        >
                            {briefing.content}
                        </ReactMarkdown>
                    ) : (
                        loading ? (
                            <p style={{ color: 'var(--color-text-tertiary)' }}>Fetching your daily summary...</p>
                        ) : (
                            <p style={{ color: 'var(--color-text-tertiary)' }}>No briefing yet. Click the refresh icon above to generate one!</p>
                        )
                    )}
                </div>
            </div>

            {!briefing && !loading && !refreshing && (
                <div className="empty-state" style={{ marginTop: 'var(--spacing-12)' }}>
                    <p style={{ fontSize: 'var(--font-size-xs)' }}>
                        Press <kbd className="kbd">Super</kbd> + <kbd className="kbd">L</kbd> to open the chat overlay
                    </p>
                </div>
            )}
        </div>
    );
}

export default Dashboard;
