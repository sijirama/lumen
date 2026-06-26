//INFO: Integrations page - Professional design
//NOTE: Collapsible cards with real brand assets

import { useState, useEffect } from 'react';
import { Check, ChevronDown, ChevronUp, FolderOpen, AlertCircle } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

//INFO: Integration type
interface Integration {
    name: string;
    enabled: boolean;
    config: string | null;
    last_sync: string | null;
    status: string;
}

//INFO: Brand Icons
const GoogleIcon = () => (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" fill="#4285F4" />
        <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853" />
        <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05" />
        <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335" />
    </svg>
);

const ObsidianIcon = () => (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M9.73 2.06c1.1.2 2.2.8 3.08 1.62.88-1.04 2.1-1.63 3.48-1.53 1.68.12 3.1 1.3 3.52 2.92.28 1.08.1 2.2-.42 3.14.7 1.13 1.15 2.45 1.12 3.8-.03 1.36-.53 2.65-1.37 3.73.5 1.35.48 2.9-.13 4.22-.5 1.08-1.42 1.9-2.5 2.37-1.33.57-2.9.5-4.22-.15-1.07.7-2.38 1-3.68.82-1.3-.18-2.5-.83-3.32-1.85-1.16-.07-2.3-.57-3.1-1.48-.98-1.1-1.43-2.6-1.2-4.05.2-1.4.15-2.88-.18-4.25-.2-1.12.08-2.3.75-3.23.6-1.1 1.25-2.15 2.02-3.13.7-1.2 1.83-2.08 3.17-2.45.68-1.2 2-1.9 3.45-1.7zm1.02 2.07c-1.1-.17-2.18.3-2.9 1.15-.3.35-.45.82-.45 1.28 0 .42.12.83.33 1.2.53.94 1.48 1.5 2.52 1.52 1.05 0 2.04-.54 2.58-1.47.25-.42.36-.92.3-1.4-.07-1.28-.9-2.3-2.13-2.43-.08 0-.17 0-.25.15z" fill="#9C27B0" opacity="0.9" />
    </svg>
);

function IntegrationsPage() {
    const [integrations, setIntegrations] = useState<Integration[]>([]);
    const [error, setError] = useState<string | null>(null);
    const [expandedMap, setExpandedMap] = useState<Record<string, boolean>>({});

    useEffect(() => {
        loadIntegrations();
    }, []);

    async function loadIntegrations() {
        try {
            const data = await invoke<Integration[]>('get_integrations');
            setIntegrations(data);

            // Pre-fill Google credentials if they exist
            const g = data.find(i => i.name === 'google');
            if (g?.config) {
                try {
                    const cfg = JSON.parse(g.config);
                    if (cfg.client_id) setGoogleClientId(cfg.client_id);
                    if (cfg.client_secret) setGoogleClientSecret(cfg.client_secret);
                } catch (e) { console.error('Failed to parse Google config', e); }
            }
        } catch (err) {
            setError(`Failed to load integrations: ${err}`);
        }
    }

    function getIntegration(name: string): Integration | undefined {
        return integrations.find(i => i.name === name);
    }

    function toggleExpand(name: string) {
        setExpandedMap(prev => ({ ...prev, [name]: !prev[name] }));
    }

    // Google Auth State
    const [googleClientId, setGoogleClientId] = useState('');
    const [googleClientSecret, setGoogleClientSecret] = useState('');
    const [isAuthenticating, setIsAuthenticating] = useState(false);

    async function handleGoogleAuth() {
        if (!googleClientId.trim() || !googleClientSecret.trim()) {
            setError('Please enter both Google Client ID and Client Secret');
            return;
        }

        setIsAuthenticating(true);
        setError(null);
        try {
            // 1. Save config first
            await invoke('save_google_config', {
                clientId: googleClientId,
                clientSecret: googleClientSecret
            });

            // 2. Start interactive auth
            await invoke('start_google_auth');
            await loadIntegrations();
        } catch (err) {
            setError(`Google Authentication failed: ${err}`);
        } finally {
            setIsAuthenticating(false);
        }
    }

    async function toggleGoogle(enabled: boolean) {
        const google = getIntegration('google');
        if (!google) return;

        // If enabling, just expand the card (auth needed)
        if (enabled) {
            setExpandedMap(prev => ({ ...prev, google: true }));
        } else {
            // Disconnect
            try {
                // Keep config, just disable
                await invoke('update_integration', {
                    integration: { name: 'google', enabled: false, config: google.config, last_sync: google.last_sync, status: 'disconnected' }
                });
                await loadIntegrations();
            } catch (err) {
                setError(`Failed to disconnect: ${err}`);
            }
        }
    }

    async function toggleObsidian(enabled: boolean) {
        if (enabled) {
            // Enabling requires selection
            selectObsidianVault();
        } else {
            // Disconnect
            try {
                await invoke('update_integration', {
                    integration: { name: 'obsidian', enabled: false, config: null, last_sync: null, status: 'disconnected' }
                });
                await loadIntegrations();
            } catch (err) {
                setError(`Failed: ${err}`);
            }
        }
    }


    async function selectObsidianVault() {
        try {
            const selectedPath = await open({ directory: true, multiple: false, title: 'Select Vault' });
            if (selectedPath && typeof selectedPath === 'string') {
                await invoke('update_integration', {
                    integration: {
                        name: 'obsidian',
                        enabled: true,
                        config: JSON.stringify({ vault_path: selectedPath }),
                        last_sync: null,
                        status: 'connected'
                    }
                });
                await loadIntegrations();
                setExpandedMap(prev => ({ ...prev, obsidian: true }));
            }
        } catch (err) {
            setError(`Failed: ${err}`);
        }
    }

    function getVaultPath(): string | null {
        const obs = getIntegration('obsidian');
        if (!obs?.config) return null;
        try {
            return JSON.parse(obs.config).vault_path;
        } catch {
            return null;
        }
    }

    const google = getIntegration('google');
    const obsidian = getIntegration('obsidian');
    const vaultPath = getVaultPath();

    return (
        <div className="animate-fade-in pb-12">
            <h2 className="mb-6 text-xl font-semibold tracking-tight">Integrations</h2>

            {error && (
                <div className="mb-4 flex items-center gap-1.5 rounded-full bg-[#fce8e6] px-3 py-1 text-xs font-medium text-error">
                    <AlertCircle size={12} />
                    {error}
                </div>
            )}

            {/* Google Services */}
            <div className="settings-card mb-4 overflow-hidden p-0">
                <div
                    onClick={() => toggleExpand('google')}
                    className="flex cursor-pointer items-center justify-between gap-3 p-4"
                >
                    <div className="flex items-center gap-3">
                        <div className="flex h-9 w-9 items-center justify-center rounded-md bg-background-tertiary">
                            <GoogleIcon />
                        </div>
                        <div>
                            <div className="text-[0.9rem] font-medium">Google Workspace</div>
                            <div className="text-xs text-foreground-secondary">
                                Connect Gmail and Calendar for context awareness.
                            </div>
                        </div>
                    </div>
                    <div className="flex items-center gap-3">
                        {google?.enabled && (
                            <div className="flex items-center gap-1 text-[0.7rem] font-medium text-success">
                                <Check size={12} /> Active
                            </div>
                        )}
                        <label className="switch scale-[0.8]" onClick={(e) => e.stopPropagation()}>
                            <input
                                type="checkbox"
                                checked={google?.enabled || false}
                                onChange={(e) => toggleGoogle(e.target.checked)}
                            />
                            <span className="slider"></span>
                        </label>
                        {expandedMap['google'] ? <ChevronUp size={16} className="text-muted" /> : <ChevronDown size={16} className="text-muted" />}
                    </div>
                </div>

                {expandedMap['google'] && (
                    <div className="border-t border-border-light bg-background-secondary p-4">
                        {!google?.enabled ? (
                            <div className="config-section">
                                <div className="mb-2 text-[0.8rem] font-semibold">API Configuration</div>
                                <p className="mb-3 text-xs text-foreground-secondary">
                                    Lumen uses local OAuth authentication. You need to provide your own Google Cloud Project credentials.
                                </p>
                                <details className="setup-details">
                                    <summary>How to get these credentials</summary>
                                    <ol>
                                        <li>Go to <a href="https://console.cloud.google.com" target="_blank" rel="noopener noreferrer">console.cloud.google.com</a> and create a new project (or pick an existing one).</li>
                                        <li><strong>APIs & Services → Library</strong> → enable <em>Gmail API</em> and <em>Google Calendar API</em>.</li>
                                        <li><strong>APIs & Services → OAuth consent screen</strong> → choose <em>External</em>, fill in the basics, and add your own Google account as a test user.</li>
                                        <li><strong>APIs & Services → Credentials → Create credentials → OAuth client ID</strong>. Application type: <em>Web application</em>.</li>
                                        <li>Under <em>Authorized redirect URIs</em>, add exactly: <code>http://127.0.0.1:18247</code></li>
                                        <li>Copy the Client ID and Client Secret into the fields below.</li>
                                    </ol>
                                    <p className="setup-note">Tip: the redirect URI must match exactly — no trailing slash, no http<strong>s</strong>.</p>
                                </details>
                                <div className="grid gap-3">
                                    <div>
                                        <label className="input-label mb-1 block">Client ID</label>
                                        <input
                                            type="text"
                                            className="input p-1.5 text-[0.8rem]"
                                            value={googleClientId}
                                            onChange={(e) => setGoogleClientId(e.target.value)}
                                            placeholder="apps.googleusercontent.com"
                                        />
                                    </div>
                                    <div>
                                        <label className="input-label mb-1 block">Client Secret</label>
                                        <input
                                            type="password"
                                            className="input p-1.5 text-[0.8rem]"
                                            value={googleClientSecret}
                                            onChange={(e) => setGoogleClientSecret(e.target.value)}
                                            placeholder="Client Secret"
                                        />
                                    </div>
                                    <div className="mt-2 flex justify-end">
                                        <button
                                            className="btn btn-primary btn-sm text-[0.8rem]"
                                            onClick={handleGoogleAuth}
                                            disabled={isAuthenticating}
                                        >
                                            {isAuthenticating ? 'Authenticating...' : 'Connect Account'}
                                        </button>
                                    </div>
                                </div>
                            </div>
                        ) : (
                            <div className="text-[0.8rem] text-foreground-secondary">
                                Connected as user. <span className="cursor-pointer underline" onClick={() => toggleGoogle(false)}>Disconnect</span>
                            </div>
                        )}
                    </div>
                )}
            </div>

            {/* Obsidian */}
            <div className="settings-card mb-4 overflow-hidden p-0">
                <div
                    onClick={() => toggleExpand('obsidian')}
                    className="flex cursor-pointer items-center justify-between gap-3 p-4"
                >
                    <div className="flex items-center gap-3">
                        <div className="flex h-9 w-9 items-center justify-center rounded-md bg-background-tertiary">
                            <ObsidianIcon />
                        </div>
                        <div>
                            <div className="text-[0.9rem] font-medium">Obsidian</div>
                            <div className="text-xs text-foreground-secondary">
                                Index your local vault for knowledge retrieval.
                            </div>
                        </div>
                    </div>
                    <div className="flex items-center gap-3">
                        {obsidian?.enabled && (
                            <div className="flex items-center gap-1 text-[0.7rem] font-medium text-success">
                                <Check size={12} /> Active
                            </div>
                        )}
                        <label className="switch scale-[0.8]" onClick={(e) => e.stopPropagation()}>
                            <input
                                type="checkbox"
                                checked={obsidian?.enabled || false}
                                onChange={(e) => toggleObsidian(e.target.checked)}
                            />
                            <span className="slider"></span>
                        </label>
                        {expandedMap['obsidian'] ? <ChevronUp size={16} className="text-muted" /> : <ChevronDown size={16} className="text-muted" />}
                    </div>
                </div>

                {expandedMap['obsidian'] && obsidian?.enabled && (
                    <div className="border-t border-border-light bg-background-secondary p-4">
                        <div className="config-section">
                            <div className="mb-2 text-[0.8rem] font-semibold">Vault Settings</div>
                            <div className="mb-3 flex items-center justify-between rounded-sm border border-border bg-background p-2 text-xs">
                                <span className="text-foreground-secondary">{vaultPath}</span>
                                <button className="cursor-pointer border-none bg-transparent text-xs text-accent" onClick={selectObsidianVault}>Change</button>
                            </div>

                            <div className="grid gap-3">
                                <div>
                                    <label className="input-label mb-1 block">Daily Notes Folder</label>
                                    <div className="flex gap-2">
                                        <input
                                            type="text"
                                            readOnly
                                            className="input flex-1 p-1.5 text-[0.8rem]"
                                            value={(() => {
                                                try { return JSON.parse(obsidian.config || '{}').daily_notes_path || '' } catch { return '' }
                                            })()}
                                            placeholder="Root"
                                        />
                                        <button className="btn btn-sm text-xs" onClick={async () => {
                                            try {
                                                const selected = await open({ directory: true, multiple: false, title: 'Select Daily Notes Folder', defaultPath: vaultPath || undefined });
                                                if (selected && typeof selected === 'string') {
                                                    let relativePath = selected;
                                                    if (vaultPath && selected.startsWith(vaultPath)) {
                                                        relativePath = selected.replace(vaultPath, '').replace(/^\//, '');
                                                    }
                                                    const config = JSON.parse(obsidian.config || '{}');
                                                    await invoke('update_integration', {
                                                        integration: {
                                                            ...obsidian,
                                                            config: JSON.stringify({ ...config, daily_notes_path: relativePath })
                                                        }
                                                    });
                                                    loadIntegrations();
                                                }
                                            } catch (err) { setError(`Failed to select folder: ${err}`); }
                                        }}>
                                            <FolderOpen size={14} />
                                        </button>
                                    </div>
                                </div>
                            </div>
                        </div>

                    </div>
                )}
            </div>

        </div>
    );
}

export default IntegrationsPage;
