import React, { useState, useEffect, useMemo } from 'react';
import { ChevronLeft, ChevronRight, ChevronDown, Loader2, Calendar as CalendarIcon, Clock, MapPin, AlignLeft } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import {
    format,
    addMonths,
    subMonths,
    startOfMonth,
    endOfMonth,
    startOfWeek,
    endOfWeek,
    isSameMonth,
    isSameDay,
    eachDayOfInterval,
    parseISO
} from 'date-fns';

interface CalendarEvent {
    id: string;
    summary: string;
    description?: string;
    location?: string;
    start: { dateTime?: string; date?: string };
    end: { dateTime?: string; date?: string };
}

interface CalendarViewProps {
    isExpanded: boolean;
    onToggleExpand: (expanded: boolean) => void;
}

const CalendarView: React.FC<CalendarViewProps> = ({ isExpanded, onToggleExpand }) => {
    const [currentDate, setCurrentDate] = useState(new Date());
    const [selectedDate, setSelectedDate] = useState(new Date());
    const [events, setEvents] = useState<CalendarEvent[]>([]);
    const [isLoading, setIsLoading] = useState(false);

    useEffect(() => {
        fetchEvents();
    }, [currentDate]);

    const fetchEvents = async () => {
        setIsLoading(true);
        try {
            const start = startOfMonth(currentDate).toISOString();
            const end = endOfMonth(currentDate).toISOString();
            const data = await invoke<CalendarEvent[]>('get_calendar_events_for_range', {
                startIso: start,
                endIso: end
            });
            setEvents(data || []);
        } catch (err) {
            console.error('Failed to fetch calendar events:', err);
        } finally {
            setIsLoading(false);
        }
    };

    const nextMonth = () => setCurrentDate(addMonths(currentDate, 1));
    const prevMonth = () => setCurrentDate(subMonths(currentDate, 1));
    const goToToday = () => {
        const today = new Date();
        setCurrentDate(today);
        setSelectedDate(today);
    };

    // Calendar Grid Logic
    const monthStart = startOfMonth(currentDate);
    const monthEnd = endOfMonth(currentDate);
    const startDate = startOfWeek(monthStart);
    const endDate = endOfWeek(monthEnd);
    const days = eachDayOfInterval({ start: startDate, end: endDate });

    const groupedEvents = useMemo(() => {
        return events.reduce((acc: Record<string, CalendarEvent[]>, event) => {
            const dateStr = event.start.dateTime || event.start.date;
            if (!dateStr) return acc;
            const key = format(parseISO(dateStr), 'yyyy-MM-dd');
            if (!acc[key]) acc[key] = [];
            acc[key].push(event);
            return acc;
        }, {});
    }, [events]);

    const colors = [
        { bg: '#e0ecff', text: '#1a73e8', border: '#1a73e8' },
        { bg: '#e6f4ea', text: '#34a853', border: '#34a853' },
        { bg: '#fef7e0', text: '#fbbc04', border: '#fbbc04' },
        { bg: '#fce8e6', text: '#ea4335', border: '#ea4335' },
        { bg: '#f3e8fd', text: '#9333ea', border: '#9333ea' },
        { bg: '#feeffa', text: '#db2777', border: '#db2777' }
    ];

    const getEventStyle = (title: string) => {
        const hash = title.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0);
        return colors[hash % colors.length];
    };

    const selectedEvents = groupedEvents[format(selectedDate, 'yyyy-MM-dd')] || [];

    return (
        <div className="sexy-calendar-container animate-fade-in">
            {/* Header: Minimal & Stylish */}
            <div className="sexy-header">
                <div
                    className="current-month-display"
                    onClick={() => onToggleExpand(!isExpanded)}
                    style={{ cursor: 'pointer' }}
                >
                    <span className="month-label">{format(currentDate, 'MMMM')}</span>
                    <span className="year-label">{format(currentDate, 'yyyy')}</span>
                    <ChevronDown
                        size={14}
                        className={`dropdown-chevron ${isExpanded ? 'rotated' : ''}`}
                    />
                </div>
                <div className="header-actions">
                    <button className="sexy-btn mini" onClick={goToToday}>Today</button>
                    <div className="nav-pair">
                        <button onClick={prevMonth} className="nav-icon-btn"><ChevronLeft size={18} /></button>
                        <button onClick={nextMonth} className="nav-icon-btn"><ChevronRight size={18} /></button>
                    </div>
                </div>
            </div>

            {/* The Grid: Airy & Modern (Collapsible) */}
            <div className={`sexy-grid-wrapper ${isExpanded ? 'expanded' : 'collapsed'}`}>
                <div className="sexy-grid">
                    <div className="weekdays-row">
                        {['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'].map(d => (
                            <div key={d} className="weekday-label">{d}</div>
                        ))}
                    </div>
                    <div className="dates-grid">
                        {days.map((day, idx) => {
                            const dateKey = format(day, 'yyyy-MM-dd');
                            const isToday = isSameDay(day, new Date());
                            const isSelected = isSameDay(day, selectedDate);
                            const isCurrentMonth = isSameMonth(day, monthStart);
                            const dayEvents = groupedEvents[dateKey] || [];

                            return (
                                <div
                                    key={idx}
                                    className={`date-cell ${!isCurrentMonth ? 'dimmed' : ''} ${isSelected ? 'selected' : ''} ${isToday ? 'today' : ''}`}
                                    onClick={() => {
                                        setSelectedDate(day);
                                        onToggleExpand(false);
                                    }}
                                >
                                    <div className="date-content">
                                        <span className="date-val">{format(day, 'd')}</span>
                                        {dayEvents.length > 0 && <div className="event-indicator" />}
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                </div>
            </div>

            {/* Event List: Modern "Bubbles" */}
            <div className="events-timeline-glow">
                <div className="timeline-header">
                    <span className="sticky-day-title">{format(selectedDate, 'EEEE, MMM do')}</span>
                    {isLoading && <Loader2 className="animate-spin opacity-50" size={14} />}
                </div>

                <div className="sexy-event-list">
                    {selectedEvents.length === 0 ? (
                        <div className="sexy-empty-state">
                            <CalendarIcon size={32} strokeWidth={1} className="opacity-10 mb-2" />
                            <p>No plans for this day</p>
                        </div>
                    ) : (
                        selectedEvents.map(event => {
                            const style = getEventStyle(event.summary);
                            const startTime = event.start.dateTime ? format(parseISO(event.start.dateTime), 'h:mm a') : 'All Day';

                            return (
                                <div key={event.id} className="sexy-pill" style={{ backgroundColor: style.bg }}>
                                    <div className="pill-main">
                                        <div className="pill-top">
                                            <span className="pill-time" style={{ color: style.text }}>
                                                <Clock size={10} className="mr-1" /> {startTime}
                                            </span>
                                            {event.location && (
                                                <span className="pill-location">
                                                    <MapPin size={10} className="mr-1" /> {event.location}
                                                </span>
                                            )}
                                        </div>
                                        <h4 className="pill-title">{event.summary}</h4>
                                        {event.description && (
                                            <p className="pill-desc">
                                                <AlignLeft size={10} className="mr-1" /> {event.description}
                                            </p>
                                        )}
                                    </div>
                                </div>
                            );
                        })
                    )}
                </div>
            </div>

            <style>{`
                .sexy-calendar-container {
                    flex: 1;
                    display: flex;
                    flex-direction: column;
                    padding: 0 2px;
                    height: 100%;
                    overflow: hidden;
                    font-family: var(--font-family-sans);
                    background: var(--color-bg-primary); /* Ensure solid background */
                    backdrop-filter: none !important;
                    -webkit-backdrop-filter: none !important;
                }

                /* Header Styling */
                .sexy-header {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    margin-bottom: var(--spacing-3);
                    padding: 0 2px;
                }

                .current-month-display {
                    display: flex;
                    align-items: baseline;
                    gap: var(--spacing-2);
                }

                .month-label {
                    font-size: 1.1rem;
                    font-weight: 800;
                    color: var(--color-text-primary);
                    letter-spacing: -0.02em;
                }

                .year-label {
                    font-size: 0.85rem;
                    font-weight: 500;
                    color: var(--color-text-tertiary);
                    opacity: 0.7;
                    margin-right: 4px;
                }

                .dropdown-chevron {
                    color: var(--color-text-tertiary);
                    opacity: 0.5;
                    transition: transform 0.3s cubic-bezier(0.4, 0, 0.2, 1);
                }
                .dropdown-chevron.rotated {
                    transform: rotate(180deg);
                }

                .header-actions {
                    display: flex;
                    align-items: center;
                    gap: var(--spacing-3);
                }

                .sexy-btn.mini {
                    background: var(--color-bg-subtle);
                    border: 1px solid var(--color-border-light);
                    padding: 3px 10px;
                    border-radius: var(--radius-full);
                    font-size: 0.65rem;
                    font-weight: 700;
                    text-transform: uppercase;
                    letter-spacing: 0.05em;
                    cursor: pointer;
                    transition: all 0.2s;
                }
                .sexy-btn.mini:hover { background: white; transform: translateY(-1px); }

                .nav-pair {
                    display: flex;
                    gap: 4px;
                }

                .nav-icon-btn {
                    background: none;
                    border: none;
                    color: var(--color-text-tertiary);
                    cursor: pointer;
                    padding: 4px;
                    border-radius: 50%;
                    transition: all 0.2s;
                    display: flex;
                }
                .nav-icon-btn:hover { background: var(--color-bg-subtle); color: var(--color-text-primary); }

                /* Grid Styling */
                .sexy-grid-wrapper {
                    overflow: hidden;
                    transition: all 0.4s cubic-bezier(0.165, 0.84, 0.44, 1);
                    opacity: 0;
                }
                .sexy-grid-wrapper.collapsed {
                    max-height: 0;
                    margin-bottom: 0;
                    pointer-events: none;
                }
                .sexy-grid-wrapper.expanded {
                    max-height: 300px;
                    opacity: 1;
                    margin-bottom: var(--spacing-4);
                    pointer-events: auto;
                }

                .sexy-grid {
                    padding-bottom: 4px;
                }

                .weekdays-row {
                    display: grid;
                    grid-template-columns: repeat(7, 1fr);
                    margin-bottom: 6px;
                }

                .weekday-label {
                    text-align: center;
                    font-size: 0.6rem;
                    font-weight: 800;
                    text-transform: uppercase;
                    color: var(--color-text-tertiary);
                    opacity: 0.5;
                    letter-spacing: 0.05em;
                }

                .dates-grid {
                    display: grid;
                    grid-template-columns: repeat(7, 1fr);
                    gap: 4px;
                }

                .date-cell {
                    aspect-ratio: 1;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    border-radius: 12px;
                    cursor: pointer;
                    transition: all 0.2s cubic-bezier(0.175, 0.885, 0.32, 1.275);
                    position: relative;
                }

                .date-content {
                    width: 100%;
                    height: 100%;
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                    gap: 2px;
                }

                .date-val {
                    font-size: 0.8rem;
                    font-weight: 600;
                    color: var(--color-text-primary);
                    z-index: 2;
                }

                .date-cell.dimmed .date-val { opacity: 0.15; }
                
                .date-cell:hover:not(.selected) {
                    background: var(--color-bg-subtle);
                }

                .date-cell.today {
                    background: rgba(167, 139, 250, 0.2); /* Slightly more opaque */
                }
                .date-cell.today .date-val {
                    color: #a78bfa;
                    font-weight: 800;
                }

                .date-cell.selected {
                    background: #a78bfa;
                    box-shadow: 0 4px 12px rgba(167, 139, 250, 0.3);
                }
                .date-cell.selected .date-val {
                    color: white;
                }

                .event-indicator {
                    width: 4px;
                    height: 4px;
                    background: #a78bfa;
                    border-radius: 50%;
                    opacity: 0.5;
                }
                .date-cell.selected .event-indicator { background: white; opacity: 1; }

                /* Event List Styling */
                .events-timeline-glow {
                    flex: 1;
                    display: flex;
                    flex-direction: column;
                    min-height: 0;
                }

                .timeline-header {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    margin-bottom: var(--spacing-3);
                }

                .sticky-day-title {
                    font-size: 0.85rem;
                    font-weight: 800;
                    color: var(--color-text-primary);
                    letter-spacing: -0.01em;
                }

                .sexy-event-list {
                    flex: 1;
                    overflow-y: auto;
                    display: flex;
                    flex-direction: column;
                    gap: var(--spacing-2);
                    padding-bottom: var(--spacing-6);
                    scrollbar-width: none;
                }
                .sexy-event-list::-webkit-scrollbar { display: none; }

                .sexy-pill {
                    padding: 10px 14px;
                    border-radius: 14px;
                    position: relative;
                    overflow: hidden;
                    transition: transform 0.2s;
                    background: var(--color-bg-secondary); /* Fallback */
                    border: 1px solid rgba(0,0,0,0.05);
                }
                .sexy-pill:hover { transform: scale(1.01); background: var(--color-bg-elevated); }


                .pill-main {
                    display: flex;
                    flex-direction: column;
                    gap: 2px;
                }

                .pill-top {
                    display: flex;
                    align-items: center;
                    gap: var(--spacing-3);
                    margin-bottom: 2px;
                }

                .pill-time, .pill-location, .pill-desc {
                    font-size: 0.6rem;
                    font-weight: 700;
                    display: flex;
                    align-items: center;
                    opacity: 0.8;
                }

                .pill-location, .pill-desc { color: var(--color-text-tertiary); font-weight: 600; opacity: 0.6; }

                .pill-title {
                    font-size: 0.75rem;
                    font-weight: 600;
                    color: var(--color-text-primary);
                    line-height: 1.2;
                }

                .pill-desc {
                    margin-top: 2px;
                    display: -webkit-box;
                    -webkit-line-clamp: 2;
                    -webkit-box-orient: vertical;
                    overflow: hidden;
                    line-height: 1.4;
                }

                .mr-1 { margin-right: 2px; }

                .sexy-empty-state {
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                    padding: var(--spacing-10) 0;
                    color: var(--color-text-tertiary);
                    font-size: 0.75rem;
                    font-weight: 600;
                }
            `}</style>
        </div>
    );
};

export default CalendarView;
