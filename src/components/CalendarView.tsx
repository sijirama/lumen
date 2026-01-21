import React from 'react';
import { ChevronLeft, ChevronRight, Video, MapPin, Clock } from 'lucide-react';

interface CalendarEvent {
    id: string;
    title: string;
    start: string;
    end: string;
    location?: string;
    color: string;
    type?: 'video' | 'physical';
}

const CalendarView: React.FC = () => {
    // Mock data based on the sleek design in the screenshot
    const events: CalendarEvent[] = [
        { id: '1', title: 'Christmas Eve', start: '12/24/25', end: 'All day', color: '#fce4ec', type: 'physical' },
        { id: '2', title: 'Jensen OOO', start: '12/24/25', end: 'All day', color: '#fff3e0', type: 'physical' },
        { id: '3', title: 'Paul WFH', start: '12/24/25', end: 'All day', color: '#efebe9', type: 'physical' },
        { id: '4', title: 'Richard comes back to Singapore', start: '12/24/25', end: 'All day', color: '#e3f2fd', type: 'physical' },
        { id: '5', title: 'Ronith OOO', start: '12/24/25', end: 'All day', color: '#e8f5e9', type: 'physical' },
        { id: '6', title: 'Flight to Dubai (EK 226)', start: '12:00 AM', end: '7:25 AM', color: '#f1f8e9', type: 'physical' },
        { id: '7', title: 'Flight: San Francisco to Dubai', start: '12:00 AM', end: '7:25 AM', color: '#f1f8e9', type: 'physical' },
    ];

    const days = ['S', 'M', 'T', 'W', 'T', 'F', 'S'];
    const currentMonth = 'December 2025';

    // Quick grid for Dec 2025 based on image
    const dates = [
        '', '', 1, 2, 3, 4, 5, 6,
        7, 8, 9, 10, 11, 12, 13,
        14, 15, 16, 17, 18, 19, 20,
        21, 22, 23, 24, 25, 26, 27,
        28, 29, 30, 31, '', '', ''
    ];

    return (
        <div className="calendar-view animate-fade-in">
            {/* Calendar Header */}
            <div className="calendar-header">
                <div className="calendar-month-nav">
                    <h2>{currentMonth}</h2>
                    <div className="calendar-nav-controls">
                        <button className="nav-btn today-btn">Today</button>
                        <div className="nav-arrows">
                            <ChevronLeft size={16} />
                            <ChevronRight size={16} />
                        </div>
                    </div>
                </div>
            </div>

            {/* Calendar Grid */}
            <div className="calendar-grid-container">
                <div className="calendar-days-row">
                    {days.map((day, i) => (
                        <div key={i} className="calendar-day-label">{day}</div>
                    ))}
                </div>
                <div className="calendar-dates-grid">
                    {dates.map((date, i) => (
                        <div key={i} className={`calendar-date-cell ${date === 24 || date === 25 ? 'has-event' : ''} ${date === '' ? 'empty' : ''}`}>
                            <span className="date-number">{date}</span>
                            {date === 25 && <div className="event-dot" />}
                        </div>
                    ))}
                </div>
            </div>

            {/* Event Timeline */}
            <div className="calendar-timeline">
                <div className="timeline-day-section">
                    <h3 className="timeline-date-sticky">Wednesday 12/24/25</h3>
                    <div className="timeline-events-list">
                        <div className="pills-row">
                            <span className="event-pill" style={{ background: '#FCE7F3', color: '#9D174D' }}>Christmas Eve</span>
                            <span className="event-pill" style={{ background: '#FFEDD5', color: '#9A3412' }}>Jensen OOO</span>
                            <span className="event-pill" style={{ background: '#f5f5f5', color: '#404040' }}>Paul WFH</span>
                        </div>
                        <div className="full-width-events">
                            <div className="event-item-block" style={{ background: '#E0F2FE', borderLeft: '4px solid #0EA5E9' }}>
                                <span className="event-item-title" style={{ color: '#0369A1' }}>Richard comes back to Singapore</span>
                            </div>
                            <div className="event-item-block" style={{ background: '#E0F2FE', borderLeft: '4px solid #0EA5E9' }}>
                                <span className="event-item-title" style={{ color: '#0369A1' }}>Ronith OOO</span>
                            </div>
                            <div className="event-item-block complex" style={{ background: '#DCFCE7', borderLeft: '4px solid #22C55E' }}>
                                <div className="event-time">12:00 AM — 7:25 AM</div>
                                <span className="event-item-title" style={{ color: '#15803D' }}>Flight to Dubai (EK 226)</span>
                            </div>
                            <div className="event-item-block complex" style={{ background: '#DCFCE7', borderLeft: '4px solid #22C55E' }}>
                                <div className="event-time">12:00 AM — 7:25 AM</div>
                                <span className="event-item-title" style={{ color: '#15803D' }}>Flight: San Francisco to Dubai</span>
                            </div>
                        </div>
                    </div>
                </div>

                <div className="timeline-day-section">
                    <h3 className="timeline-date-sticky">Thursday 12/25/25</h3>
                    <div className="timeline-events-list">
                        <div className="pills-row">
                            <span className="event-pill" style={{ background: '#FCE7F3', color: '#9D174D' }}>Christmas Day</span>
                            <span className="event-pill" style={{ background: '#FFEDD5', color: '#9A3412' }}>Jensen OOO</span>
                        </div>
                        <div className="pills-row">
                            <span className="event-pill" style={{ background: '#E0F2FE', color: '#0369A1' }}>Niso's Birthday</span>
                            <span className="event-pill" style={{ background: '#f5f5f5', color: '#404040' }}>Paul WFH</span>
                            <span className="event-pill" style={{ background: '#DCFCE7', color: '#15803D' }}>Ronith OOO</span>
                        </div>
                    </div>
                </div>
            </div>

            <style>{`
                .calendar-view {
                    flex: 1;
                    display: flex;
                    flex-direction: column;
                    gap: var(--spacing-4);
                    height: 100%;
                    overflow-y: auto;
                    scrollbar-width: none;
                }
                .calendar-view::-webkit-scrollbar { display: none; }

                .calendar-header {
                    padding: 0 var(--spacing-2);
                }

                .calendar-month-nav {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                }

                .calendar-month-nav h2 {
                    font-size: 1.1rem;
                    font-weight: 700;
                    color: var(--color-text-primary);
                }

                .calendar-nav-controls {
                    display: flex;
                    align-items: center;
                    gap: var(--spacing-2);
                }

                .nav-btn {
                    padding: 4px 10px;
                    border-radius: var(--radius-full);
                    border: 1px solid var(--color-border);
                    background: white;
                    font-size: 0.75rem;
                    font-weight: 500;
                    cursor: pointer;
                }

                .nav-arrows {
                    display: flex;
                    gap: var(--spacing-1);
                    color: var(--color-text-tertiary);
                    cursor: pointer;
                }

                .calendar-grid-container {
                    padding: var(--spacing-2);
                }

                .calendar-days-row {
                    display: grid;
                    grid-template-columns: repeat(7, 1fr);
                    margin-bottom: var(--spacing-2);
                }

                .calendar-day-label {
                    text-align: center;
                    font-size: 0.7rem;
                    font-weight: 600;
                    color: var(--color-text-tertiary);
                    text-transform: uppercase;
                }

                .calendar-dates-grid {
                    display: grid;
                    grid-template-columns: repeat(7, 1fr);
                    gap: 2px;
                }

                .calendar-date-cell {
                    height: 38px;
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    justify-content: center;
                    position: relative;
                    font-size: 0.85rem;
                    font-weight: 400;
                    border-radius: var(--radius-md);
                    cursor: pointer;
                    color: var(--color-text-primary);
                    transition: background 0.2s;
                }

                .calendar-date-cell:hover:not(.empty) {
                    background: var(--color-bg-hover);
                }

                .calendar-date-cell.empty { cursor: default; }

                .calendar-date-cell.has-event {
                    font-weight: 600;
                }

                .event-dot {
                    width: 4px;
                    height: 4px;
                    background: var(--color-accent);
                    border-radius: 50%;
                    position: absolute;
                    bottom: 6px;
                }

                .calendar-timeline {
                    display: flex;
                    flex-direction: column;
                    gap: var(--spacing-6);
                    padding: 0 var(--spacing-2);
                }

                .timeline-date-sticky {
                    font-size: 0.95rem;
                    font-weight: 600;
                    margin-bottom: var(--spacing-3);
                    color: var(--color-text-primary);
                }

                .timeline-events-list {
                    display: flex;
                    flex-direction: column;
                    gap: var(--spacing-2);
                }

                .pills-row {
                    display: flex;
                    flex-wrap: wrap;
                    gap: var(--spacing-2);
                    margin-bottom: var(--spacing-1);
                }

                .event-pill {
                    padding: 4px 12px;
                    border-radius: var(--radius-lg);
                    font-size: 0.75rem;
                    font-weight: 500;
                }

                .full-width-events {
                    display: flex;
                    flex-direction: column;
                    gap: var(--spacing-2);
                }

                .event-item-block {
                    padding: 10px 12px;
                    border-radius: var(--radius-md);
                    font-size: 0.85rem;
                    display: flex;
                    flex-direction: column;
                    gap: 2px;
                }

                .event-item-block.complex {
                    padding: 12px;
                }

                .event-time {
                    font-size: 0.7rem;
                    opacity: 0.8;
                    margin-bottom: 2px;
                }

                .event-item-title {
                    font-weight: 600;
                }
            `}</style>
        </div>
    );
};

export default CalendarView;
