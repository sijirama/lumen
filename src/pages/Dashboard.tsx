import { useState, useEffect } from 'react';
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

                <button
                    className={`refresh-btn ${refreshing ? 'spinning' : ''}`}
                    onClick={refreshBriefing}
                    disabled={refreshing}
                    title="Refresh briefing"
                    style={{ padding: 'var(--spacing-2)', opacity: 0.6 }}
                >
                    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M23 4v6h-6"></path>
                        <path d="M1 20v-6h6"></path>
                        <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
                    </svg>
                </button>
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
                        <ReactMarkdown remarkPlugins={[remarkGfm]}>
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
