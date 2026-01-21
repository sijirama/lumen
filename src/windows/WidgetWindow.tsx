import { useState, useEffect } from 'react';
import { Sparkles, Calendar } from 'lucide-react';

function WidgetWindow() {
    const [time, setTime] = useState(new Date());

    useEffect(() => {
        const timer = setInterval(() => setTime(new Date()), 1000);
        return () => clearInterval(timer);
    }, []);

    const formatTime = (date: Date) => {
        return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    };

    return (
        <div className="widget-container">
            <div className="widget-content">
                <div className="widget-left">
                    <div className="widget-icon">
                        <Sparkles size={16} />
                    </div>
                    <div className="widget-info">
                        <span className="widget-label">Lumen is active</span>
                        <span className="widget-value">{formatTime(time)}</span>
                    </div>
                </div>
                <div className="widget-right">
                    <Calendar size={14} style={{ opacity: 0.5 }} />
                </div>
            </div>
        </div>
    );
}

export default WidgetWindow;
