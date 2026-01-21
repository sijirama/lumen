import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

export default function SnipperWindow() {
    // The original state for snipping functionality
    const [startPos, setStartPos] = useState<{ x: number; y: number } | null>(null);
    const [currentPos, setCurrentPos] = useState<{ x: number; y: number } | null>(null);
    const [isDragging, setIsDragging] = useState(false);

    useEffect(() => {
        // Add class to body for specific styling
        document.body.classList.add('snipper-window');
        document.documentElement.classList.add('snipper-window');

        // Listen for escape key
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') {
                cancelSnip();
            }
        };

        window.addEventListener('keydown', handleKeyDown);

        return () => {
            document.body.classList.remove('snipper-window');
            document.documentElement.classList.remove('snipper-window');
            window.removeEventListener('keydown', handleKeyDown);
        };
    }, []);

    const cancelSnip = async () => {
        await invoke('close_snipper');
    };

    const handleMouseDown = (e: React.MouseEvent) => {
        setIsDragging(true);
        setStartPos({ x: e.clientX, y: e.clientY });
        setCurrentPos({ x: e.clientX, y: e.clientY });
    };

    const handleMouseMove = (e: React.MouseEvent) => {
        if (isDragging) {
            setCurrentPos({ x: e.clientX, y: e.clientY });
        }
    };

    const handleMouseUp = async () => {
        if (!startPos || !currentPos) return;
        setIsDragging(false);

        // Calculate normalized rect
        const x = Math.min(startPos.x, currentPos.x);
        const y = Math.min(startPos.y, currentPos.y);
        const width = Math.abs(currentPos.x - startPos.x);
        const height = Math.abs(currentPos.y - startPos.y);

        // Ignore tiny accidental clicks
        if (width < 10 || height < 10) {
            setStartPos(null);
            setCurrentPos(null);
            return;
        }

        try {
            // Send coordinates to backend to crop the cached screenshot using Physical Pixels
            // Note: We might need to adjust for DPR if the Rust side doesn't handle it
            // For now, let's send logical client coordinates and let backend handle scaling or 
            // assume 1:1 if we are lucky. 
            // ACTUALLY: Tauri's window positioning is logical. Arboard screenshots are physical.
            // We will handle DPR in backend or frontend. Let's send the raw client vals first.

            await invoke('capture_region', { x, y, width, height });
        } catch (error) {
            console.error("Failed to capture region", error);
            cancelSnip();
        }
    };

    // Calculate box styles
    const getSelectionStyle = () => {
        if (!startPos || !currentPos) return {};

        const left = Math.min(startPos.x, currentPos.x);
        const top = Math.min(startPos.y, currentPos.y);
        const width = Math.abs(currentPos.x - startPos.x);
        const height = Math.abs(currentPos.y - startPos.y);

        return {
            left,
            top,
            width,
            height
        };
    };

    return (
        <div
            className="snipper-container"
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
        >
            {startPos && currentPos && (
                <div className="selection-box" style={getSelectionStyle()} />
            )}

            {/* Helper text */}
            {!isDragging && !startPos && (
                <div style={{
                    position: 'absolute',
                    top: '50%',
                    left: '50%',
                    transform: 'translate(-50%, -50%)',
                    color: 'white',
                    background: 'rgba(0,0,0,0.6)',
                    padding: '8px 16px',
                    borderRadius: '8px',
                    pointerEvents: 'none',
                    fontSize: '14px',
                    fontWeight: 500
                }}>
                    Click and drag to capture
                </div>
            )}
        </div>
    );
}
