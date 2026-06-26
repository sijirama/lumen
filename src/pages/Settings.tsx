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
        <div className="animate-fade-in pb-12">
            <div className="mb-6 flex items-center justify-between">
                <h2 className="text-xl font-semibold tracking-tight">Settings</h2>
                {success && (
                    <div className="flex items-center gap-1.5 rounded-full bg-background-tertiary px-3 py-1 text-xs font-medium text-success">
                        <CheckCircle2 size={12} />
                        {success}
                    </div>
                )}
                {error && (
                    <div className="flex items-center gap-1.5 rounded-full bg-[#fce8e6] px-3 py-1 text-xs font-medium text-error">
                        <AlertCircle size={12} />
                        {error}
                    </div>
                )}
            </div>

            {/* General Settings */}
            <section className="mb-6">
                <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-foreground-tertiary">
                    General
                </h4>
                <div className="settings-card px-4 py-3">
                    <div className="settings-row">
                        <div className="settings-row-info">
                            <span className="settings-row-title text-[0.9rem]">Launch on Startup</span>
                            <span className="settings-row-description text-[0.8rem]">Start Lumen automatically when you log in.</span>
                        </div>
                        <label className="switch scale-[0.8]">
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
            <section className="mb-6">
                <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-foreground-tertiary">
                    Personalization
                </h4>
                <div className="settings-card p-4">
                    <div className="mb-3">
                        <label className="mb-1 block text-[0.8rem] font-medium text-foreground-secondary">Display Name</label>
                        <input
                            type="text"
                            className="input px-2.5 py-1.5 text-[0.9rem]"
                            value={displayName}
                            onChange={(e) => setDisplayName(e.target.value)}
                            placeholder="Your name"
                        />
                    </div>
                    <div className="mb-4">
                        <label className="mb-1 block text-[0.8rem] font-medium text-foreground-secondary">Home Location</label>
                        <input
                            type="text"
                            className="input px-2.5 py-1.5 text-[0.9rem]"
                            value={location}
                            onChange={(e) => setLocation(e.target.value)}
                            placeholder="e.g. Lagos, London"
                        />
                        <div className="mt-1 text-xs text-foreground-tertiary">
                            Used for local weather updates.
                        </div>
                    </div>
                    <div className="flex justify-end">
                        <button className="btn btn-primary btn-sm text-[0.8rem]" onClick={saveProfile} disabled={saving}>
                            Save Changes
                        </button>
                    </div>
                </div>
            </section>

            {/* Hotkey */}
            <section className="mb-6">
                <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-foreground-tertiary">
                    Shortcuts
                </h4>
                <div className="settings-card p-4">

                    {/* Main Activation */}
                    <div className="mb-4">
                        <label className="mb-1.5 block text-[0.8rem] font-medium text-foreground-secondary">Activation Hotkey</label>
                        <div className="flex items-center gap-2">
                            <div className="flex gap-1">
                                {['Super', 'Ctrl', 'Alt', 'Shift'].map(mod => (
                                    <button
                                        key={mod}
                                        onClick={() => toggleModifier(mod, true)}
                                        className={`cursor-pointer rounded-sm border px-2.5 py-1 text-xs transition-colors ${hotkeyModifiers.includes(mod) ? 'border-accent bg-accent text-white' : 'border-border bg-transparent text-foreground-secondary'}`}
                                    >
                                        {mod}
                                    </button>
                                ))}
                            </div>
                            <span className="text-[0.9rem] text-foreground-tertiary">+</span>
                            <div className="relative">
                                <input
                                    type="text"
                                    value={hotkeyKey}
                                    onChange={(e) => setHotkeyKey(e.target.value.toUpperCase())}
                                    maxLength={1}
                                    className="w-10 rounded-sm border border-border p-1 text-center text-[0.9rem] font-bold outline-none"
                                />
                            </div>
                        </div>
                    </div>

                    {/* Snipper Shortcut */}
                    <div className="mb-3">
                        <label className="mb-1.5 block text-[0.8rem] font-medium text-foreground-secondary">Snipping Tool</label>
                        <div className="flex items-center gap-2">
                            <div className="flex gap-1">
                                {['Super', 'Ctrl', 'Alt', 'Shift'].map(mod => (
                                    <button
                                        key={mod}
                                        onClick={() => toggleModifier(mod, false)}
                                        className={`cursor-pointer rounded-sm border px-2.5 py-1 text-xs transition-colors ${snipperModifiers.includes(mod) ? 'border-accent bg-accent text-white' : 'border-border bg-transparent text-foreground-secondary'}`}
                                    >
                                        {mod}
                                    </button>
                                ))}
                            </div>
                            <span className="text-[0.9rem] text-foreground-tertiary">+</span>
                            <div className="relative">
                                <input
                                    type="text"
                                    value={snipperKey}
                                    onChange={(e) => setSnipperKey(e.target.value.toUpperCase())}
                                    maxLength={1}
                                    className="w-10 rounded-sm border border-border p-1 text-center text-[0.9rem] font-bold outline-none"
                                />
                            </div>
                        </div>
                    </div>

                    <div className="mt-3 flex items-center justify-end">
                        <button className="btn btn-primary btn-sm text-[0.8rem]" onClick={saveHotkey} disabled={saving}>
                            Update Shortcuts
                        </button>
                    </div>
                </div>
            </section>

            {/* API Key */}
            <section className="mb-6">
                <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-foreground-tertiary">
                    Intelligence
                </h4>
                <div className="settings-card p-4">
                    <div className="settings-row mb-3">
                        <div className="settings-row-info">
                            <span className="settings-row-title text-[0.9rem]">Gemini API Key</span>
                        </div>
                        {geminiKeyConfigured && (
                            <div className="flex items-center gap-1 rounded-sm bg-[rgba(52,168,83,0.1)] px-2 py-0.5 text-xs font-semibold text-success">
                                Active
                            </div>
                        )}
                    </div>

                    <div className="mb-3">
                        <input
                            type="password"
                            className="input px-2.5 py-1.5 font-mono text-[0.9rem]"
                            value={geminiApiKey}
                            onChange={(e) => setGeminiApiKey(e.target.value)}
                            placeholder={geminiKeyConfigured ? '••••••••••••••••••••••••' : 'Paste API Key'}
                        />
                    </div>
                    <div className="flex items-center justify-between">
                        <a
                            href="https://aistudio.google.com/apikey"
                            target="_blank"
                            rel="noreferrer"
                            className="text-xs text-foreground-secondary no-underline"
                        >
                            Get APi key
                        </a>
                        <button className="btn btn-primary btn-sm text-[0.8rem]" onClick={saveApiKey} disabled={saving || !geminiApiKey.trim()}>
                            Save Key
                        </button>
                    </div>

                </div>
            </section>

            {/* Action Approvals */}
            <section className="mb-6">
                <button
                    onClick={() => setApprovalsOpen(o => !o)}
                    className="flex w-full cursor-pointer items-center justify-between border-none bg-transparent pb-2"
                >
                    <h4 className="m-0 text-xs font-semibold uppercase tracking-wider text-foreground-tertiary">
                        Action Approvals
                    </h4>
                    <ChevronDown
                        size={14}
                        className={`text-foreground-tertiary transition-transform ${approvalsOpen ? 'rotate-180' : 'rotate-0'}`}
                    />
                </button>
                {approvalsOpen && (
                    <div className="settings-card px-4 py-3">
                        <p className="mb-3 text-xs leading-[1.4] text-foreground-tertiary">
                            When enabled, Lumen will ask for your confirmation before running that tool.
                        </p>
                        {ALL_TOOLS.map(({ key, label, description }, i) => (
                            <div
                                key={key}
                                className={`settings-row ${i < ALL_TOOLS.length - 1 ? 'mb-3' : ''}`}
                            >
                                <div className="settings-row-info">
                                    <span className="settings-row-title text-sm">{label}</span>
                                    <span className="settings-row-description text-[0.78rem]">{description}</span>
                                </div>
                                <label className="switch scale-[0.8] shrink-0">
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
                <div className="settings-card border-none bg-transparent p-3 shadow-none">
                    <p className="mb-1 text-xs text-muted">
                        Database Location
                    </p>
                    <code className="inline-block rounded-sm bg-[rgba(0,0,0,0.03)] px-1.5 py-1 text-[0.7rem] text-foreground-tertiary">
                        {databasePath}
                    </code>
                </div>
            </section>
        </div>
    );
}

export default SettingsPage;
