//INFO: Memory page — see and prune what Lumen has learned about you.
//NOTE: The privacy/trust surface for the Generative-Agents memory subsystem.
//      Nothing here is uploaded anywhere; it all lives in the local SQLite DB.

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertCircle, Trash2, Brain, Plus, Pencil, Check, X, ChevronLeft, ChevronRight } from 'lucide-react';
import Button from '../components/ui/Button';

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
const PAGE_SIZE = 7;

function MemoryPage() {
    const [memories, setMemories] = useState<MemoryRow[]>([]);
    const [filter, setFilter] = useState<TypeFilter>('all');
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [newMemory, setNewMemory] = useState('');
    const [adding, setAdding] = useState(false);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [editText, setEditText] = useState('');
    const [page, setPage] = useState(1);

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
    const pageCount = Math.max(1, Math.ceil(shown.length / PAGE_SIZE));
    const currentPage = Math.min(page, pageCount);
    const pageStart = (currentPage - 1) * PAGE_SIZE;
    const pagedMemories = shown.slice(pageStart, pageStart + PAGE_SIZE);

    return (
        <div className="admin-page animate-fade-in">
            <div className="admin-page-header">
                <h2>Memory</h2>
            </div>

            <p className="admin-page-copy">
                What Lumen has learned about you across conversations. All local — nothing is uploaded.
            </p>

            {/* Teach Lumen something directly */}
            <div className="admin-inline-form">
                <input
                    type="text"
                    className="input"
                    value={newMemory}
                    onChange={(e) => setNewMemory(e.target.value)}
                    onKeyDown={(e) => { if (e.key === 'Enter') addMemory(); }}
                    placeholder="Teach Lumen a fact — e.g. “I’m allergic to peanuts”"
                />
                <Button
                    size="sm"
                    onClick={addMemory}
                    disabled={adding || !newMemory.trim()}
                >
                    <Plus size={14} />
                    {adding ? 'Saving…' : 'Remember'}
                </Button>
            </div>

            {error && (
                <div className="mb-4 flex max-w-full items-center gap-1.5 rounded-full bg-[#fce8e6] px-3 py-1 text-xs font-medium text-error">
                    <AlertCircle size={12} />
                    <span className="min-w-0 break-words">{error}</span>
                </div>
            )}

            {/* Type filter chips */}
            <div className="admin-pill-row">
                {TYPE_FILTERS.map(t => {
                    const count = t === 'all' ? memories.length : memories.filter(m => m.memory_type === t).length;
                    if (t !== 'all' && count === 0) return null;
                    const active = filter === t;
                    return (
                        <button
                            key={t}
                            onClick={() => { setFilter(t); setPage(1); }}
                            className={`admin-pill ${active ? 'active' : ''}`}
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
                <>
                <div className="memory-list">
                    {pagedMemories.map(m => (
                        <div key={m.id} className="memory-row">
                            <span className="memory-type-pill">
                                {m.memory_type.replace('_', ' ')}
                            </span>
                            {editingId === m.id ? (
                                <>
                                    <textarea
                                        value={editText}
                                        onChange={(e) => setEditText(e.target.value)}
                                        autoFocus
                                        rows={2}
                                        className="memory-edit-input"
                                    />
                                    <button onClick={() => saveEdit(m.id)} title="Save" className="icon-button text-success">
                                        <Check size={15} />
                                    </button>
                                    <button onClick={() => setEditingId(null)} title="Cancel" className="icon-button text-muted hover:text-foreground">
                                        <X size={15} />
                                    </button>
                                </>
                            ) : (
                                <>
                                    <span className="memory-content">
                                        {m.content}
                                    </span>
                                    <div className="memory-actions">
                                        <span title="Importance" className="memory-score">
                                            ★ {m.importance.toFixed(0)}
                                        </span>
                                        <button
                                            onClick={() => { setEditingId(m.id); setEditText(m.content); }}
                                            title="Edit"
                                            className="icon-button text-muted hover:text-foreground"
                                        >
                                            <Pencil size={13} />
                                        </button>
                                        <button
                                            onClick={() => deleteMemory(m.id)}
                                            title="Forget this"
                                            className="icon-button text-muted hover:text-error"
                                        >
                                            <Trash2 size={14} />
                                        </button>
                                    </div>
                                </>
                            )}
                        </div>
                    ))}
                </div>
                {shown.length > PAGE_SIZE && (
                    <div className="pagination-row">
                        <span>{pageStart + 1}-{Math.min(pageStart + PAGE_SIZE, shown.length)} of {shown.length}</span>
                        <div className="pagination-actions">
                            <button className="icon-button" onClick={() => setPage(p => Math.max(1, p - 1))} disabled={currentPage === 1}>
                                <ChevronLeft size={15} />
                            </button>
                            <span>{currentPage} / {pageCount}</span>
                            <button className="icon-button" onClick={() => setPage(p => Math.min(pageCount, p + 1))} disabled={currentPage === pageCount}>
                                <ChevronRight size={15} />
                            </button>
                        </div>
                    </div>
                )}
                </>
            )}
        </div>
    );
}

export default MemoryPage;
