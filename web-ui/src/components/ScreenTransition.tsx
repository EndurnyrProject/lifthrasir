import { ReactNode, useEffect, useState } from 'react';
import './ScreenTransition.css';

interface ScreenTransitionProps {
  children: ReactNode;
  transitionKey: string;
}

/**
 * ScreenTransition - Smooth fade transitions between screens
 *
 * Wraps screen content and applies fade-in/fade-out animations
 * when the transitionKey changes.
 *
 * @param children - Screen content to render
 * @param transitionKey - Unique key for current screen (triggers transition on change)
 *
 * @example
 * ```tsx
 * <ScreenTransition transitionKey={currentScreen}>
 *   {currentScreen === 'login' && <Login />}
 *   {currentScreen === 'server_selection' && <ServerSelection />}
 * </ScreenTransition>
 * ```
 */
export function ScreenTransition({ children, transitionKey }: ScreenTransitionProps) {
  const [displayChildren, setDisplayChildren] = useState(children);
  const [isAnimating, setIsAnimating] = useState(false);

  useEffect(() => {
    // Trigger fade-out animation
    setIsAnimating(true);

    // Wait for fade-out to complete, then update content
    const fadeOutTimer = setTimeout(() => {
      setDisplayChildren(children);
      setIsAnimating(false);
    }, 300); // Match CSS transition duration

    return () => clearTimeout(fadeOutTimer);
  }, [transitionKey, children]);

  return (
    <div className={`screen-transition ${isAnimating ? 'screen-transition-out' : 'screen-transition-in'}`}>
      {displayChildren}
    </div>
  );
}
