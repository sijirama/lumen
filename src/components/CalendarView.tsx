import React, { useState, useEffect } from 'react';
import { ChevronLeft, ChevronRight, Loader2 } from 'lucide-react';
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
    eachDayOfInterval
} from 'date-fns';

interface CalendarEvent {
    id: string;
    summary: string;
    description?: string;
    location?: string;
    start: { dateTime?: string; date?: string };
    end: { dateTime?: string; date?: string };
}

const CalendarView: React.FC = () => {
    const [currentDate, setCurrentDate] = useState(new Date());
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
            setEvents(data);
        } catch (err) {
            console.error('Failed to fetch calendar events:', err);
        } finally {
            setIsLoading(false);
        }
    };

    const nextMonth = () => setCurrentDate(addMonths(currentDate, 1));
    const prevMonth = () => setCurrentDate(subMonths(currentDate, 1));
    const goToToday = () => setCurrentDate(new Date());

    // Calendar Grid Logic
    const monthStart = startOfMonth(currentDate);
    const monthEnd = endOfMonth(currentDate);
    const startDate = startOfWeek(monthStart);
    const endDate = endOfWeek(monthEnd);

    const days = eachDayOfInterval({ start: startDate, end: endDate });

    const groupedEvents = events.reduce((acc: any, event) => {
        const dateKey = event.start.dateTime
            ? format(new Date(event.start.dateTime), 'yyyy-MM-dd')
            : event.start.date;
        if (!dateKey) return acc;
        if (!acc[dateKey]) acc[dateKey] = [];
        acc[dateKey].push(event);
        return acc;
    }, {});

    const timelineDates = Object.keys(groupedEvents).sort();

    return (
        <div className="calendar-view animate-fade-in">
            {/* Header */}
            <div className="calendar-header">
                <div className="calendar-month-nav">
                    <h2>{format(currentDate, 'MMMM yyyy')}</h2>
                    <div className="calendar-nav-controls">
                        <button className="nav-btn today-btn" onClick={goToToday}>Today</button>
                        <div className="nav-arrows">
                            <button onClick={prevMonth} className="icon-btn"><ChevronLeft size={16} /></button>
                            <button onClick={nextMonth} className="icon-btn"><ChevronRight size={16} /></button>
                        </div>
                    </div>
                </div>
            </div>

            {/* Grid */}
            <div className="calendar-grid-container">
                <div className="calendar-days-row">
                    {['S', 'M', 'T', 'W', 'T', 'F', 'S'].map((day, i) => (
                        <div key={i} className="calendar-day-label">{day}</div>
                    ))}
                </div>
                <div className="calendar-dates-grid">
                    {days.map((day, i) => {
                        const dateKey = format(day, 'yyyy-MM-dd');
                        const hasEvents = groupedEvents[dateKey]?.length > 0;
                        const isToday = isSameDay(day, new Date());

                        return (
                            <div
                                key={i}
                                className={`calendar-date-cell ${!isSameMonth(day, monthStart) ? 'out-of-month' : ''} ${isToday ? 'is-today' : ''}`}
                            >
                                <span className="date-number">{format(day, 'd')}</span>
                                {hasEvents && <div className="event-dot" />}
                            </div>
                        );
                    })}
                </div>
            </div>

            {/* Timeline */}
            <div className="calendar-timeline">
                {isLoading && events.length === 0 ? (
                    <div className="calendar-loading">
                        <Loader2 className="animate-spin" size={20} />
                    </div>
                ) : timelineDates.length === 0 ? (
                    <div className="no-events">No events scheduled.</div>
                ) : (
                    timelineDates.map(dateKey => (
                        <div key={dateKey} className="timeline-day-section">
                            <h3 className="timeline-date-sticky">
                                {format(new Date(dateKey), 'EEEE MM/dd/yy')}
                            </h3>
                            <div className="timeline-events-list">
                                {groupedEvents[dateKey].map((event: CalendarEvent) => {
                                    const timeStr = event.start.dateTime
                                        ? format(new Date(event.start.dateTime), 'h:mm a')
                                        : 'All day';

                                    const colors = [
                                        { bg: '#FCE7F3', text: '#9D174D' }, // Pink
                                        { bg: '#FFEDD5', text: '#9A3412' }, // Orange
                                        { bg: '#E0F2FE', text: '#0369A1' }, // Blue
                                        { bg: '#DCFCE7', text: '#15803D' }, // Green
                                        { bg: '#F3E8FF', text: '#6B21A8' }, // Purple
                                        { bg: '#F1F5F9', text: '#334155' }  // Slate
                                    ];
                                    const colorIndex = Math.abs(event.summary.split('').reduce((a, b) => a + b.charCodeAt(0), 0));
                                    const style = colors[colorIndex % colors.length];

                                    return (
                                        <div
                                            key={event.id}
                                            className="event-pill-full"
                                            style={{ backgroundColor: style.bg, color: style.text }}
                                        >
                                            <span className="event-pill-time">{timeStr}</span>
                                            <span className="event-pill-title">{event.summary}</span>
                                        </div>
                                    );
                                })}
                            </div>
                        </div>
                    ))
                )}
            </div>

            <style>{`
                .calendar-view {
                    flex: 1;
                    display: flex;
                    flex-direction: column;
                    gap: var(--spacing-2);
                    height: 100%;
                    overflow-y: auto;
                    scrollbar-width: none;
                    padding: 0 var(--spacing-1); /* Minimized X padding */
                }
                .calendar-view::-webkit-scrollbar { display: none; }

                .calendar-header {
                    margin-bottom: var(--spacing-1);
                }

                .calendar-month-nav {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                }

                .calendar-month-nav h2 {
                    font-size: 0.95rem;
                    font-weight: 700;
                    color: var(--color-text-primary);
                }

                .calendar-nav-controls {
                    display: flex;
                    align-items: center;
                    gap: var(--spacing-2);
                }

                .nav-btn {
                    padding: 2px 8px;
                    border-radius: var(--radius-full);
                    border: 1px solid var(--color-border-light);
                    background: white;
                    font-size: 0.65rem;
                    font-weight: 600;
                    cursor: pointer;
                }
                
                .icon-btn {
                    background: none;
                    border: none;
                    color: var(--color-text-tertiary);
                    cursor: pointer;
                    display: flex;
                    align-items: center;
                    padding: 2px;
                }

                .nav-arrows {
                    display: flex;
                    gap: var(--spacing-1);
                }

                .calendar-grid-container {
                    padding: 0;
                    margin-bottom: var(--spacing-1);
                }

                .calendar-days-row {
                    display: grid;
                    grid-template-columns: repeat(7, 1fr);
                    margin-bottom: var(--spacing-1);
                }

                .calendar-day-label {
                    text-align: center;
                    font-size: 0.6rem;
                    font-weight: 700;
                    color: var(--color-text-tertiary);
                }

                .calendar-dates-grid {
                    display: grid;
                    grid-template-columns: repeat(7, 1fr);
                    gap: 1px;
                }

                .calendar-date-cell {
                    height: 32px;
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                    position: relative;
                    font-size: 0.75rem;
                    border-radius: var(--radius-md);
                    cursor: pointer;
                    color: var(--color-text-primary);
                }

                .calendar-date-cell.is-today {
                    background: var(--color-accent-light);
                    color: var(--color-accent);
                    font-weight: 700;
                }

                .calendar-date-cell.out-of-month {
                    opacity: 0.2;
                }

                .event-dot {
                    width: 3px;
                    height: 3px;
                    background: var(--color-accent);
                    border-radius: 50%;
                    position: absolute;
                    bottom: 3px;
                }

                .calendar-timeline {
                    display: flex;
                    flex-direction: column;
                    gap: var(--spacing-3);
                    margin-top: 0; /* Minimized margin from calendar */
                }

                .timeline-date-sticky {
                    font-size: 0.8rem;
                    font-weight: 600;
                    margin-bottom: var(--spacing-1);
                    color: var(--color-text-tertiary);
                    background: var(--color-bg-primary);
                    padding: 2px 0;
                }

                .timeline-events-list {
                    display: flex;
                    flex-direction: column;
                    gap: var(--spacing-1);
                }

                .event-pill-full {
                    padding: 6px 10px;
                    border-radius: var(--radius-md);
                    font-size: 0.7rem; /* Reduced font size */
                    font-weight: 500;
                    display: flex;
                    flex-direction: column;
                    gap: 0;
                    border: 1px solid rgba(0,0,0,0.02);
                }

                .event-pill-time {
                    font-size: 0.6rem;
                    opacity: 0.7;
                    font-weight: 600;
                    margin-bottom: -1px;
                }

                .event-pill-title {
                    line-height: 1.1;
                    font-weight: 600;
                }

                .calendar-loading, .no-events {
                    padding: var(--spacing-6);
                    text-align: center;
                    color: var(--color-text-tertiary);
                    font-size: 0.75rem;
                }
            `}</style>
        </div>
    );
};

export default CalendarView;
