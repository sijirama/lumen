import type { ButtonHTMLAttributes, ReactNode } from 'react';

type ButtonVariant = 'default' | 'secondary' | 'ghost' | 'danger';
type ButtonSize = 'sm' | 'md' | 'icon';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
    children: ReactNode;
    variant?: ButtonVariant;
    size?: ButtonSize;
}

function Button({
    children,
    className = '',
    variant = 'default',
    size = 'md',
    type = 'button',
    ...props
}: ButtonProps) {
    return (
        <button
            type={type}
            className={`ui-button ui-button-${variant} ui-button-${size} ${className}`.trim()}
            {...props}
        >
            {children}
        </button>
    );
}

export default Button;
