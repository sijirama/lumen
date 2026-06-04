import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
import { CheckCircle2, AlertCircle, ChevronDown } from 'lucide-react';

//INFO: All tools with approval metadata
const ALL_TOOLS: { key: string; label: string; description: string; defaultApproval: boolean }[] = [
    // Google / comms — high risk, default ON
    { key: 'send_email',              label: 'Send email',                  description: 'Send an email via Gmail.',                          defaultApproval: true },
    { key: 'delete_calendar_event',   label: 'Delete calendar event',       description: 'Permanently remove a Google Calendar event.',       defaultApproval: true },
    // Calendar — medium risk, default OFF
    { key: 'create_calendar_event',   label: 'Create calendar event',       description: 'Add a new event to Google Calendar.',               defaultApproval: false },
    { key: 'get_google_calendar_events', label: 'Read calendar events',    description: 'List upcoming events from Google Calendar.',        defaultApproval: false },
    { key: 'get_unread_emails',       label: 'Read emails',                 description: 'Fetch unread or recent Gmail messages.',            defaultApproval: false },
    // File ops — write risk ON, reads OFF
    { key: 'write_file',              label: 'Write file',                  description: 'Write or overwrite a file on disk.',                defaultApproval: true },
    { key: 'edit_file_line',          label: 'Edit file line',              description: 'Replace a specific line inside a file.',            defaultApproval: true },
    { key: 'insert_at_line',          label: 'Insert at line',              description: 'Insert a new line into a file.',                   defaultApproval: true },
    { key: 'delete_file_line',        label: 'Delete file line',            description: 'Remove a specific line from a file.',              defaultApproval: true },
    { key: 'read_file',               label: 'Read file',                   description: 'Read the contents of a local file.',               defaultApproval: false },
    { key: 'read_file_lines',         label: 'Read file lines',             description: 'Read a specific line range from a file.',          defaultApproval: false },
    { key: 'list_files',              label: 'List files',                  description: 'List files in a directory.',                       defaultApproval: false },
    { key: 'search_notes',            label: 'Search notes',                description: 'Search Obsidian vault markdown files.',             defaultApproval: false },
    { key: 'grep_file',               label: 'Grep file',                   description: 'Search for a pattern inside a file.',              defaultApproval: false },
    { key: 'get_file_metadata',       label: 'Get file metadata',           description: 'Read size and timestamps for a file.',             defaultApproval: false },
    { key: 'search_filesystem',       label: 'Search filesystem',           description: 'Recursively find files matching a pattern.',       defaultApproval: false },
    { key: 'get_obsidian_vault_info', label: 'Get vault info',              description: 'Read Obsidian vault root path.',                   defaultApproval: false },
    // Core tools — all OFF by default
    { key: 'get_weather',             label: 'Get weather',                 description: 'Fetch current weather for a location.',           defaultApproval: false },
    { key: 'take_screenshot',         label: 'Take screenshot',             description: 'Capture the primary screen.',                     defaultApproval: false },
    { key: 'search_clipboard',        label: 'Search clipboard',            description: 'Search recent clipboard history.',                 defaultApproval: false },
];

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
    snipper_modifier_keys: string[];
    snipper_key: string;
    snipper_enabled: boolean;
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

    // Main Hotkey
    const [hotkeyModifiers, setHotkeyModifiers] = useState<string[]>(['Super']);
    const [hotkeyKey, setHotkeyKey] = useState('L');

    // Snipper Hotkey
    const [snipperModifiers, setSnipperModifiers] = useState<string[]>(['Super', 'Shift']);
    const [snipperKey, setSnipperKey] = useState('S');

    const [geminiApiKey, setGeminiApiKey] = useState('');
    const [geminiKeyConfigured, setGeminiKeyConfigured] = useState(false);
    const [databasePath, setDatabasePath] = useState('');
    const [autostartEnabled, setAutostartEnabled] = useState(false);

    // Action approval settings — map of tool key → requires approval
    const [approvals, setApprovals] = useState<Record<string, boolean>>({});
    const [approvalsOpen, setApprovalsOpen] = useState(false);

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
                // Load Snipper config if available (backend defaults handle nulls)
                if (hotkey.snipper_modifier_keys) setSnipperModifiers(hotkey.snipper_modifier_keys);
                if (hotkey.snipper_key) setSnipperKey(hotkey.snipper_key);
            }

            const geminiStatus = await invoke<ApiKeyStatus>('get_api_key_status', { provider: 'gemini' });
            setGeminiKeyConfigured(geminiStatus.is_configured);

            const dbPath = await invoke<string>('get_database_path');
            setDatabasePath(dbPath);

            const isAutostart = await isEnabled();
            setAutostartEnabled(isAutostart);

            // Load approval settings for all tools
            const loaded: Record<string, boolean> = {};
            await Promise.all(ALL_TOOLS.map(async ({ key, defaultApproval }) => {
                try {
                    const val = await invoke<string | null>('get_app_setting', { key: `approval_${key}` });
                    loaded[key] = val !== null ? val === 'true' : defaultApproval;
                } catch {
                    loaded[key] = defaultApproval;
                }
            }));
            setApprovals(loaded);
        } catch (err) {
            setError(`Failed to load settings: ${err}`);
        }
    }

    async function toggleApproval(toolKey: string) {
        const current = approvals[toolKey] ?? false;
        const next = !current;
        setApprovals(prev => ({ ...prev, [toolKey]: next }));
        try {
            await invoke('save_app_setting', { key: `approval_${toolKey}`, value: String(next) });
        } catch (err) {
            setApprovals(prev => ({ ...prev, [toolKey]: current }));
            setError(`Failed to save setting: ${err}`);
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
            await invoke('update_hotkey', {
                request: {
                    modifier_keys: hotkeyModifiers,
                    key: hotkeyKey,
                    enabled: true,
                    snipper_modifier_keys: snipperModifiers,
                    snipper_key: snipperKey,
                    snipper_enabled: true
                }
            });
            setSuccess('Shortcuts saved (restart to apply)');
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

    function toggleModifier(mod: string, isMain: boolean) {
        if (isMain) {
            if (hotkeyModifiers.includes(mod)) {
                setHotkeyModifiers(hotkeyModifiers.filter(m => m !== mod));
            } else {
                setHotkeyModifiers([...hotkeyModifiers, mod]);
            }
        } else {
            if (snipperModifiers.includes(mod)) {
                setSnipperModifiers(snipperModifiers.filter(m => m !== mod));
            } else {
                setSnipperModifiers([...snipperModifiers, mod]);
            }
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

                    {/* Main Activation */}
                    <div style={{ marginBottom: 'var(--spacing-4)' }}>
                        <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 500, marginBottom: '6px', color: 'var(--color-text-secondary)' }}>Activation Hotkey</label>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                            <div style={{ display: 'flex', gap: '4px' }}>
                                {['Super', 'Ctrl', 'Alt', 'Shift'].map(mod => (
                                    <button
                                        key={mod}
                                        onClick={() => toggleModifier(mod, true)}
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

                    {/* Snipper Shortcut */}
                    <div style={{ marginBottom: 'var(--spacing-3)' }}>
                        <label style={{ display: 'block', fontSize: '0.8rem', fontWeight: 500, marginBottom: '6px', color: 'var(--color-text-secondary)' }}>Snipping Tool</label>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                            <div style={{ display: 'flex', gap: '4px' }}>
                                {['Super', 'Ctrl', 'Alt', 'Shift'].map(mod => (
                                    <button
                                        key={mod}
                                        onClick={() => toggleModifier(mod, false)}
                                        style={{
                                            padding: '4px 10px',
                                            fontSize: '0.75rem',
                                            borderRadius: '4px',
                                            border: '1px solid',
                                            borderColor: snipperModifiers.includes(mod) ? 'var(--color-accent)' : 'var(--color-border)',
                                            background: snipperModifiers.includes(mod) ? 'var(--color-accent)' : 'transparent',
                                            color: snipperModifiers.includes(mod) ? 'white' : 'var(--color-text-secondary)',
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
                                    value={snipperKey}
                                    onChange={(e) => setSnipperKey(e.target.value.toUpperCase())}
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

                    <div style={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', marginTop: 'var(--spacing-3)' }}>
                        <button className="btn btn-primary btn-sm" onClick={saveHotkey} disabled={saving} style={{ fontSize: '0.8rem' }}>
                            Update Shortcuts
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

            {/* Action Approvals */}
            <section style={{ marginBottom: 'var(--spacing-6)' }}>
                <button
                    onClick={() => setApprovalsOpen(o => !o)}
                    style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        width: '100%',
                        background: 'none',
                        border: 'none',
                        padding: '0 0 var(--spacing-2) 0',
                        cursor: 'pointer',
                    }}
                >
                    <h4 style={{
                        fontSize: '0.75rem',
                        textTransform: 'uppercase',
                        letterSpacing: '0.05em',
                        color: 'var(--color-text-tertiary)',
                        fontWeight: 600,
                        margin: 0,
                    }}>
                        Action Approvals
                    </h4>
                    <ChevronDown
                        size={14}
                        style={{
                            color: 'var(--color-text-tertiary)',
                            transform: approvalsOpen ? 'rotate(180deg)' : 'rotate(0deg)',
                            transition: 'transform 0.2s',
                        }}
                    />
                </button>
                {approvalsOpen && (
                    <div className="settings-card" style={{ padding: 'var(--spacing-3) var(--spacing-4)' }}>
                        <p style={{ fontSize: '0.75rem', color: 'var(--color-text-tertiary)', marginBottom: 'var(--spacing-3)', lineHeight: 1.4 }}>
                            When enabled, Lumen will ask for your confirmation before running that tool.
                        </p>
                        {ALL_TOOLS.map(({ key, label, description }, i) => (
                            <div
                                key={key}
                                className="settings-row"
                                style={i < ALL_TOOLS.length - 1 ? { marginBottom: 'var(--spacing-3)' } : {}}
                            >
                                <div className="settings-row-info">
                                    <span className="settings-row-title" style={{ fontSize: '0.875rem' }}>{label}</span>
                                    <span className="settings-row-description" style={{ fontSize: '0.78rem' }}>{description}</span>
                                </div>
                                <label className="switch" style={{ transform: 'scale(0.8)', flexShrink: 0 }}>
                                    <input
                                        type="checkbox"
                                        checked={approvals[key] ?? false}
                                        onChange={() => toggleApproval(key)}
                                    />
                                    <span className="slider"></span>
                                </label>
                            </div>
                        ))}
                    </div>
                )}
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
