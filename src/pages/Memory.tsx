//INFO: Memory page — see and prune what Lumen has learned about you.
//NOTE: The privacy/trust surface for the Generative-Agents memory subsystem.
//      Nothing here is uploaded anywhere; it all lives in the local SQLite DB.

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertCircle, Trash2, Brain, Plus, Pencil, Check, X } from 'lucide-react';

interface MemoryRow {
    id: string;
    memory_type: string;
    content: string;
    importance: number;
    created_at: string;
    access_count: number;
}

const TYPE_FILTERS = ['all', 'observation', 'preference', 'entity', 'reflection', 'daily_summary'] as const;
type TypeFilter = typeof TYPE_FILTERS[number];

function MemoryPage() {
    const [memories, setMemories] = useState<MemoryRow[]>([]);
    const [filter, setFilter] = useState<TypeFilter>('all');
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [newMemory, setNewMemory] = useState('');
    const [adding, setAdding] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [editText, setEditText] = useState('');

    useEffect(() => {
        loadMemories();
    }, []);

    async function loadMemories() {
        setLoading(true);
        try {
            const rows = await invoke<MemoryRow[]>('get_memories', { limit: 500 });
            setMemories(rows);
            setError(null);
        } catch (err) {
            setError(`Failed to load memories: ${err}`);
        } finally {
            setLoading(false);
        }
    }

    async function deleteMemory(id: string) {
        try {
            await invoke('delete_memory', { id });
            setMemories(prev => prev.filter(m => m.id !== id));
        } catch (err) {
            setError(`Failed to delete memory: ${err}`);
        }
    }

    async function forgetEverything() {
        if (!confirm('Forget everything Lumen has learned about you? This wipes all memories and cannot be undone.')) return;
        try {
            await invoke('clear_all_memories');
            setMemories([]);
        } catch (err) {
            setError(`Failed to clear memories: ${err}`);
        }
    }

    async function addMemory() {
        const content = newMemory.trim();
        if (!content || adding) return;
        setAdding(true);
        try {
            await invoke('add_memory', { content });
            setNewMemory('');
            await loadMemories();
        } catch (err) {
            setError(`Failed to add memory: ${err}`);
        } finally {
            setAdding(false);
        }
    }

    async function saveEdit(id: string) {
        const content = editText.trim();
        if (!content) return;
        try {
            await invoke('update_memory', { id, content });
            setMemories(prev => prev.map(m => (m.id === id ? { ...m, content } : m)));
            setEditingId(null);
        } catch (err) {
            setError(`Failed to update memory: ${err}`);
        }
    }

    const shown = filter === 'all' ? memories : memories.filter(m => m.memory_type === filter);

    return (
        <div className="animate-fade-in" style={{ paddingBottom: 'var(--spacing-12)' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 'var(--spacing-6)' }}>
                <h2 style={{ fontSize: '1.25rem', fontWeight: 600, letterSpacing: '-0.025em', margin: 0 }}>Memory</h2>
                <button
                    className="btn btn-sm"
                    onClick={forgetEverything}
                    disabled={memories.length === 0}
                    style={{ fontSize: '0.75rem', color: 'var(--color-error)', display: 'flex', alignItems: 'center', gap: '4px' }}
                >
                    <Trash2 size={13} />
                    Forget everything
                </button>
            </div>

            <p style={{ fontSize: '0.8rem', color: 'var(--color-text-secondary)', marginBottom: 'var(--spacing-4)' }}>
                What Lumen has learned about you across conversations. All local — nothing is uploaded.
            </p>

            {/* Teach Lumen something directly */}
            <div style={{ display: 'flex', gap: '8px', marginBottom: 'var(--spacing-4)' }}>
                <input
                    type="text"
                    className="input"
                    value={newMemory}
                    onChange={(e) => setNewMemory(e.target.value)}
                    onKeyDown={(e) => { if (e.key === 'Enter') addMemory(); }}
                    placeholder="Teach Lumen a fact — e.g. “I’m allergic to peanuts”"
                    style={{ flex: 1, fontSize: '0.82rem', padding: '8px 10px' }}
                />
                <button
                    className="btn btn-primary btn-sm"
                    onClick={addMemory}
                    disabled={adding || !newMemory.trim()}
                    style={{ fontSize: '0.78rem', display: 'flex', alignItems: 'center', gap: '4px' }}
                >
                    <Plus size={14} />
                    {adding ? 'Saving…' : 'Remember'}
                </button>
            </div>

            {error && (
                <div style={{
                    padding: '4px 12px', background: '#fce8e6', borderRadius: 'var(--radius-full)',
                    color: 'var(--color-error)', fontSize: '0.75rem', fontWeight: 500,
                    display: 'flex', alignItems: 'center', gap: '6px', marginBottom: 'var(--spacing-4)'
                }}>
                    <AlertCircle size={12} />
                    {error}
                </div>
            )}

            {/* Type filter chips */}
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px', marginBottom: 'var(--spacing-4)' }}>
                {TYPE_FILTERS.map(t => {
                    const count = t === 'all' ? memories.length : memories.filter(m => m.memory_type === t).length;
                    if (t !== 'all' && count === 0) return null;
                    return (
                        <button
                            key={t}
                            onClick={() => setFilter(t)}
                            style={{
                                fontSize: '0.72rem', fontWeight: 600, padding: '3px 10px', borderRadius: 'var(--radius-full)',
                                cursor: 'pointer', textTransform: 'capitalize',
                                border: '1px solid ' + (filter === t ? 'var(--color-accent)' : 'var(--color-border)'),
                                background: filter === t ? 'var(--color-accent-light)' : 'transparent',
                                color: filter === t ? 'var(--color-accent)' : 'var(--color-text-secondary)',
                            }}
                        >
                            {t.replace('_', ' ')} {count > 0 && <span style={{ opacity: 0.6 }}>· {count}</span>}
                        </button>
                    );
                })}
            </div>

            {loading ? (
                <p style={{ fontSize: '0.8rem', color: 'var(--color-text-muted)' }}>Loading…</p>
            ) : shown.length === 0 ? (
                <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '8px', padding: 'var(--spacing-12) 0', color: 'var(--color-text-muted)' }}>
                    <Brain size={28} style={{ opacity: 0.5 }} />
                    <p style={{ fontSize: '0.85rem' }}>Nothing yet — Lumen learns as you chat.</p>
                </div>
            ) : (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    {shown.map(m => (
                        <div key={m.id} style={{
                            display: 'flex', alignItems: 'flex-start', gap: '10px', padding: '10px 12px',
                            background: 'var(--color-bg-secondary)', border: '1px solid var(--color-border-light)',
                            borderRadius: 'var(--radius-md)',
                        }}>
                            <span style={{
                                fontSize: '0.58rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.04em',
                                color: 'var(--color-accent)', background: 'var(--color-accent-light)',
                                padding: '2px 6px', borderRadius: '4px', whiteSpace: 'nowrap', marginTop: '1px',
                            }}>
                                {m.memory_type.replace('_', ' ')}
                            </span>
                            {editingId === m.id ? (
                                <>
                                    <textarea
                                        value={editText}
                                        onChange={(e) => setEditText(e.target.value)}
                                        autoFocus
                                        rows={2}
                                        style={{ flex: 1, fontSize: '0.82rem', padding: '4px 6px', border: '1px solid var(--color-accent)', borderRadius: '4px', resize: 'vertical', fontFamily: 'inherit', lineHeight: 1.4 }}
                                    />
                                    <button onClick={() => saveEdit(m.id)} title="Save" style={{ border: 'none', background: 'none', color: 'var(--color-success)', cursor: 'pointer', padding: '0 2px', marginTop: '1px', display: 'flex' }}>
                                        <Check size={15} />
                                    </button>
                                    <button onClick={() => setEditingId(null)} title="Cancel" style={{ border: 'none', background: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', padding: '0 2px', marginTop: '1px', display: 'flex' }}>
                                        <X size={15} />
                                    </button>
                                </>
                            ) : (
                                <>
                                    <span style={{ flex: 1, fontSize: '0.82rem', color: 'var(--color-text-primary)', lineHeight: 1.4 }}>
                                        {m.content}
                                    </span>
                                    <span title="Importance" style={{ fontSize: '0.68rem', color: 'var(--color-text-tertiary)', marginTop: '2px', whiteSpace: 'nowrap' }}>
                                        ★ {m.importance.toFixed(0)}
                                    </span>
                                    <button
                                        onClick={() => { setEditingId(m.id); setEditText(m.content); }}
                                        title="Edit"
                                        style={{ border: 'none', background: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', padding: '0 2px', marginTop: '1px', display: 'flex' }}
                                    >
                                        <Pencil size={13} />
                                    </button>
                                    <button
                                        onClick={() => deleteMemory(m.id)}
                                        title="Forget this"
                                        style={{ border: 'none', background: 'none', color: 'var(--color-text-muted)', cursor: 'pointer', padding: '0 2px', marginTop: '1px', display: 'flex' }}
                                    >
                                        <Trash2 size={14} />
                                    </button>
                                </>
                            )}
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
}

export default MemoryPage;
