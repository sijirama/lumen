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
        <div className="animate-fade-in pb-12">
            <div className="mb-6 flex items-center justify-between">
                <h2 className="m-0 text-xl font-semibold tracking-tight">Memory</h2>
                <button
                    className="btn btn-sm flex items-center gap-1 text-xs text-error"
                    onClick={forgetEverything}
                    disabled={memories.length === 0}
                >
                    <Trash2 size={13} />
                    Forget everything
                </button>
            </div>

            <p className="mb-4 text-[0.8rem] text-foreground-secondary">
                What Lumen has learned about you across conversations. All local — nothing is uploaded.
            </p>

            {/* Teach Lumen something directly */}
            <div className="mb-4 flex gap-2">
                <input
                    type="text"
                    className="input flex-1 px-2.5 py-2 text-[0.82rem]"
                    value={newMemory}
                    onChange={(e) => setNewMemory(e.target.value)}
                    onKeyDown={(e) => { if (e.key === 'Enter') addMemory(); }}
                    placeholder="Teach Lumen a fact — e.g. “I’m allergic to peanuts”"
                />
                <button
                    className="btn btn-primary btn-sm flex items-center gap-1 text-[0.78rem]"
                    onClick={addMemory}
                    disabled={adding || !newMemory.trim()}
                >
                    <Plus size={14} />
                    {adding ? 'Saving…' : 'Remember'}
                </button>
            </div>

            {error && (
                <div className="mb-4 flex items-center gap-1.5 rounded-full bg-[#fce8e6] px-3 py-1 text-xs font-medium text-error">
                    <AlertCircle size={12} />
                    {error}
                </div>
            )}

            {/* Type filter chips */}
            <div className="mb-4 flex flex-wrap gap-1.5">
                {TYPE_FILTERS.map(t => {
                    const count = t === 'all' ? memories.length : memories.filter(m => m.memory_type === t).length;
                    if (t !== 'all' && count === 0) return null;
                    const active = filter === t;
                    return (
                        <button
                            key={t}
                            onClick={() => setFilter(t)}
                            className={`cursor-pointer rounded-full border px-2.5 py-[3px] text-[0.72rem] font-semibold capitalize transition-colors ${active ? 'border-accent bg-accent-light text-accent' : 'border-border bg-transparent text-foreground-secondary hover:border-accent'}`}
                        >
                            {t.replace('_', ' ')} {count > 0 && <span className="opacity-60">· {count}</span>}
                        </button>
                    );
                })}
            </div>

            {loading ? (
                <p className="text-[0.8rem] text-muted">Loading…</p>
            ) : shown.length === 0 ? (
                <div className="flex flex-col items-center gap-2 py-12 text-muted">
                    <Brain size={28} className="opacity-50" />
                    <p className="text-[0.85rem]">Nothing yet — Lumen learns as you chat.</p>
                </div>
            ) : (
                <div className="flex flex-col gap-2">
                    {shown.map(m => (
                        <div key={m.id} className="flex items-start gap-2.5 rounded-md border border-border-light bg-background-secondary px-3 py-2.5">
                            <span className="mt-px whitespace-nowrap rounded-sm bg-accent-light px-1.5 py-0.5 text-[0.58rem] font-bold uppercase tracking-[0.04em] text-accent">
                                {m.memory_type.replace('_', ' ')}
                            </span>
                            {editingId === m.id ? (
                                <>
                                    <textarea
                                        value={editText}
                                        onChange={(e) => setEditText(e.target.value)}
                                        autoFocus
                                        rows={2}
                                        className="flex-1 resize-y rounded-sm border border-accent px-1.5 py-1 font-[inherit] text-[0.82rem] leading-[1.4]"
                                    />
                                    <button onClick={() => saveEdit(m.id)} title="Save" className="mt-px flex cursor-pointer border-none bg-transparent px-0.5 text-success">
                                        <Check size={15} />
                                    </button>
                                    <button onClick={() => setEditingId(null)} title="Cancel" className="mt-px flex cursor-pointer border-none bg-transparent px-0.5 text-muted hover:text-foreground">
                                        <X size={15} />
                                    </button>
                                </>
                            ) : (
                                <>
                                    <span className="flex-1 text-[0.82rem] leading-[1.4] text-foreground">
                                        {m.content}
                                    </span>
                                    <span title="Importance" className="mt-0.5 whitespace-nowrap text-[0.68rem] text-foreground-tertiary">
                                        ★ {m.importance.toFixed(0)}
                                    </span>
                                    <button
                                        onClick={() => { setEditingId(m.id); setEditText(m.content); }}
                                        title="Edit"
                                        className="mt-px flex cursor-pointer border-none bg-transparent px-0.5 text-muted transition-colors hover:text-foreground"
                                    >
                                        <Pencil size={13} />
                                    </button>
                                    <button
                                        onClick={() => deleteMemory(m.id)}
                                        title="Forget this"
                                        className="mt-px flex cursor-pointer border-none bg-transparent px-0.5 text-muted transition-colors hover:text-error"
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
