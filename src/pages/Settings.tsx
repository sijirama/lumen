import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';

//INFO: Types
interface UserProfile {
    display_name: string;
    location: string | null;
    theme: string;
}

interface HotkeyConfig {
    modifier_keys: string[];
    key: string;
    enabled: boolean;
}

interface ApiKeyStatus {
    provider: string;
    is_configured: boolean;
    masked_key: string | null;
}

function SettingsPage() {
    //INFO: State
    const [displayName, setDisplayName] = useState('');
    const [location, setLocation] = useState('');
    const [hotkeyModifiers, setHotkeyModifiers] = useState<string[]>(['Super']);
    const [hotkeyKey, setHotkeyKey] = useState('L');
    const [geminiApiKey, setGeminiApiKey] = useState('');
    const [geminiKeyConfigured, setGeminiKeyConfigured] = useState(false);
    const [databasePath, setDatabasePath] = useState('');
    const [autostartEnabled, setAutostartEnabled] = useState(false);

    //INFO: UI state
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState<string | null>(null);

    //INFO: Load settings on mount
    useEffect(() => {
        loadSettings();
    }, []);

    //INFO: Clear success message after 3s
    useEffect(() => {
        if (success) {
            const timer = setTimeout(() => setSuccess(null), 3000);
            return () => clearTimeout(timer);
        }
    }, [success]);

    async function loadSettings() {
        try {
            const profile = await invoke<UserProfile | null>('get_profile');
            if (profile) {
                setDisplayName(profile.display_name);
                setLocation(profile.location || '');
            }

            const hotkey = await invoke<HotkeyConfig | null>('get_hotkey');
            if (hotkey) {
                setHotkeyModifiers(hotkey.modifier_keys);
                setHotkeyKey(hotkey.key);
            }

            const geminiStatus = await invoke<ApiKeyStatus>('get_api_key_status', { provider: 'gemini' });
            setGeminiKeyConfigured(geminiStatus.is_configured);

            const dbPath = await invoke<string>('get_database_path');
            setDatabasePath(dbPath);

            const isAutostart = await isEnabled();
            setAutostartEnabled(isAutostart);
        } catch (err) {
            setError(`Failed to load settings: ${err}`);
        }
    }

    async function toggleAutostart() {
        try {
            if (autostartEnabled) {
                await disable();
            } else {
                await enable();
            }
            setAutostartEnabled(!autostartEnabled);
            setSuccess(autostartEnabled ? 'Auto-launch disabled' : 'Auto-launch enabled');
        } catch (err) {
            setError(`Failed to update auto-launch: ${err}`);
        }
    }

    async function saveProfile() {
        setSaving(true);
        setError(null);
        try {
            await invoke('update_profile', { request: { display_name: displayName, location: location || null, theme: 'light' } });
            setSuccess('Profile saved');
        } catch (err) {
            setError(`Failed to save profile: ${err}`);
        } finally {
            setSaving(false);
        }
    }

    async function saveHotkey() {
        setSaving(true);
        setError(null);
        try {
            await invoke('update_hotkey', { request: { modifier_keys: hotkeyModifiers, key: hotkeyKey, enabled: true } });
            setSuccess('Hotkey saved (restart to apply)');
        } catch (err) {
            setError(`Failed to save hotkey: ${err}`);
        } finally {
            setSaving(false);
        }
    }

    async function saveApiKey() {
        if (!geminiApiKey.trim()) return;
        setSaving(true);
        setError(null);
        try {
            await invoke('update_api_key', { request: { provider: 'gemini', api_key: geminiApiKey } });
            setGeminiApiKey('');
            setGeminiKeyConfigured(true);
            setSuccess('API key saved');
        } catch (err) {
            setError(`Failed to save API key: ${err}`);
        } finally {
            setSaving(false);
        }
    }

    function toggleModifier(mod: string) {
        if (hotkeyModifiers.includes(mod)) {
            setHotkeyModifiers(hotkeyModifiers.filter(m => m !== mod));
        } else {
            setHotkeyModifiers([...hotkeyModifiers, mod]);
        }
    }

    return (
        <div className="animate-fade-in" style={{ paddingBottom: 'var(--spacing-12)' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 'var(--spacing-6)' }}>
                <h2 style={{ fontSize: '1.25rem', fontWeight: 600, letterSpacing: '-0.025em' }}>Settings</h2>
                {success && (
                    <div style={{
                        padding: '4px 12px',
                        background: 'var(--color-bg-tertiary)',
                        borderRadius: 'var(--radius-full)',
                        color: 'var(--color-success)',
                        fontSize: '0.75rem',
                        fontWeight: 500,
                        display: 'flex',
                        alignItems: 'center',
                        gap: '6px'
                    }}>
                        <CheckCircle2 size={12} />
                        {success}
                    </div>
                )}
                {error && (
                    <div style={{
                        padding: '4px 12px',
                        background: '#fce8e6',
                        borderRadius: 'var(--radius-full)',
                        color: 'var(--color-error)',
                        fontSize: '0.75rem',
                        fontWeight: 500,
                        display: 'flex',
                        alignItems: 'center',
                        gap: '6px'
                    }}>
                        <AlertCircle size={12} />
                        {error}
                    </div>
                )}
            </div>

            {/* General Settings */}
            <section style={{ marginBottom: 'var(--spacing-6)' }}>
                <h4 style={{
                    fontSize: '0.75rem',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em',
                    color: 'var(--color-text-tertiary)',
                    marginBottom: 'var(--spacing-2)',
                    fontWeight: 600
                }}>
                    General
                </h4>
                <div className="settings-card" style={{ padding: 'var(--spacing-3) var(--spacing-4)' }}>
                    <div className="settings-row">
                        <div className="settings-row-info">
                            <span className="settings-row-title" style={{ fontSize: '0.9rem' }}>Launch on Startup</span>
                            <span className="settings-row-description" style={{ fontSize: '0.8rem' }}>Start Lumen automatically when you log in.</span>
                        </div>
                        <label className="switch" style={{ transform: 'scale(0.8)' }}>
                            <input
                                type="checkbox"
                                checked={autostartEnabled}
                                onChange={toggleAutostart}
                            />
                            <span className="slider"></span>
                        </label>
                    </div>

                </div>
            </section>

            {/* Profile */}
            <section style={{ marginBottom: 'var(--spacing-6)' }}>
                <h4 style={{
                    fontSize: '0.75rem',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em',
                    color: 'var(--color-text-tertiary)',
                    marginBottom: 'var(--spacing-2)',
                    fontWeight: 600
                }}>
                    Personalization
                </h4>
                <div className="settings-card" style={{ padding: 'var(--spacing-4)' }}>
                    <div style={{ marginBottom: 'var(--spacing-3)' }}>
                        <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 500, marginBottom: '4px', color: 'var(--color-text-secondary)' }}>Display Name</label>
                        <input
                            type="text"
                            className="input"
                            value={displayName}
                            onChange={(e) => setDisplayName(e.target.value)}
                            placeholder="Your name"
                            style={{ fontSize: '0.9rem', padding: '6px 10px' }}
                        />
                    </div>
                    <div style={{ marginBottom: 'var(--spacing-4)' }}>
                        <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 500, marginBottom: '4px', color: 'var(--color-text-secondary)' }}>Home Location</label>
                        <input
                            type="text"
                            className="input"
                            value={location}
                            onChange={(e) => setLocation(e.target.value)}
                            placeholder="e.g. Lagos, London"
                            style={{ fontSize: '0.9rem', padding: '6px 10px' }}
                        />
                        <div style={{ fontSize: '0.75rem', color: 'var(--color-text-tertiary)', marginTop: '4px' }}>
                            Used for local weather updates.
                        </div>
                    </div>
                    <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
                        <button className="btn btn-primary btn-sm" onClick={saveProfile} disabled={saving} style={{ fontSize: '0.8rem' }}>
                            Save Changes
                        </button>
                    </div>
                </div>
            </section>

            {/* Hotkey */}
            <section style={{ marginBottom: 'var(--spacing-6)' }}>
                <h4 style={{
                    fontSize: '0.75rem',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em',
                    color: 'var(--color-text-tertiary)',
                    marginBottom: 'var(--spacing-2)',
                    fontWeight: 600
                }}>
                    Shortcuts
                </h4>
                <div className="settings-card" style={{ padding: 'var(--spacing-4)' }}>
                    <div style={{ marginBottom: 'var(--spacing-3)' }}>
                        <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 500, marginBottom: '6px', color: 'var(--color-text-secondary)' }}>Activation Hotkey</label>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                            <div style={{ display: 'flex', gap: '4px' }}>
                                {['Super', 'Ctrl', 'Alt', 'Shift'].map(mod => (
                                    <button
                                        key={mod}
                                        onClick={() => toggleModifier(mod)}
                                        style={{
                                            padding: '4px 10px',
                                            fontSize: '0.75rem',
                                            borderRadius: '4px',
                                            border: '1px solid',
                                            borderColor: hotkeyModifiers.includes(mod) ? 'var(--color-accent)' : 'var(--color-border)',
                                            background: hotkeyModifiers.includes(mod) ? 'var(--color-accent)' : 'transparent',
                                            color: hotkeyModifiers.includes(mod) ? 'white' : 'var(--color-text-secondary)',
                                            cursor: 'pointer',
                                            transition: 'all 0.2s'
                                        }}
                                    >
                                        {mod}
                                    </button>
                                ))}
                            </div>
                            <span style={{ color: 'var(--color-text-tertiary)', fontSize: '0.9rem' }}>+</span>
                            <div style={{ position: 'relative' }}>
                                <input
                                    type="text"
                                    value={hotkeyKey}
                                    onChange={(e) => setHotkeyKey(e.target.value.toUpperCase())}
                                    maxLength={1}
                                    style={{
                                        width: '40px',
                                        textAlign: 'center',
                                        fontWeight: 'bold',
                                        fontSize: '0.9rem',
                                        padding: '4px',
                                        borderRadius: '4px',
                                        border: '1px solid var(--color-border)',
                                        outline: 'none'
                                    }}
                                />
                            </div>
                        </div>
                    </div>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 'var(--spacing-3)' }}>
                        <span style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)' }}>
                            Active: <span style={{ fontWeight: 500 }}>{hotkeyModifiers.join('+')}+{hotkeyKey}</span>
                        </span>
                        <button className="btn btn-primary btn-sm" onClick={saveHotkey} disabled={saving} style={{ fontSize: '0.8rem' }}>
                            Update
                        </button>
                    </div>
                </div>
            </section>

            {/* API Key */}
            <section style={{ marginBottom: 'var(--spacing-6)' }}>
                <h4 style={{
                    fontSize: '0.75rem',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em',
                    color: 'var(--color-text-tertiary)',
                    marginBottom: 'var(--spacing-2)',
                    fontWeight: 600
                }}>
                    Intelligence
                </h4>
                <div className="settings-card" style={{ padding: 'var(--spacing-4)' }}>
                    <div className="settings-row" style={{ marginBottom: 'var(--spacing-3)' }}>
                        <div className="settings-row-info">
                            <span className="settings-row-title" style={{ fontSize: '0.9rem' }}>Gemini API Key</span>
                        </div>
                        {geminiKeyConfigured && (
                            <div style={{ display: 'flex', alignItems: 'center', gap: '4px', color: 'var(--color-success)', fontSize: '0.75rem', fontWeight: 600, background: 'rgba(52, 168, 83, 0.1)', padding: '2px 8px', borderRadius: '4px' }}>
                                Active
                            </div>
                        )}
                    </div>

                    <div style={{ marginBottom: 'var(--spacing-3)' }}>
                        <input
                            type="password"
                            className="input"
                            value={geminiApiKey}
                            onChange={(e) => setGeminiApiKey(e.target.value)}
                            placeholder={geminiKeyConfigured ? '••••••••••••••••••••••••' : 'Paste API Key'}
                            style={{ fontSize: '0.9rem', padding: '6px 10px', fontFamily: 'monospace' }}
                        />
                    </div>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                        <a
                            href="https://aistudio.google.com/apikey"
                            target="_blank"
                            rel="noreferrer"
                            style={{ fontSize: '0.75rem', color: 'var(--color-text-secondary)', textDecoration: 'none' }}
                        >
                            Get APi key
                        </a>
                        <button className="btn btn-primary btn-sm" onClick={saveApiKey} disabled={saving || !geminiApiKey.trim()} style={{ fontSize: '0.8rem' }}>
                            Save Key
                        </button>
                    </div>
                </div>
            </section>

            {/* Data */}
            <section>
                <div className="settings-card" style={{ padding: 'var(--spacing-3)', background: 'transparent', border: 'none', boxShadow: 'none' }}>
                    <p style={{ fontSize: '0.75rem', color: 'var(--color-text-muted)', marginBottom: '4px' }}>
                        Database Location
                    </p>
                    <code style={{
                        display: 'inline-block',
                        fontSize: '0.7rem',
                        color: 'var(--color-text-tertiary)',
                        background: 'rgba(0,0,0,0.03)',
                        padding: '4px 6px',
                        borderRadius: '4px',
                    }}>
                        {databasePath}
                    </code>
                </div>
            </section>
        </div>
    );
}

export default SettingsPage;
