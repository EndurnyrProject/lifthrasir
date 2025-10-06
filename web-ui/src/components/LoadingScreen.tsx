import { LoadingSpinner } from './LoadingSpinner';
import './LoadingScreen.css';

interface LoadingScreenProps {
  message?: string;
  backgroundUrl?: string | null;
  containerClassName?: string;
}

/**
 * Beautiful loading screen with spinner
 *
 * Shows a spinner and loading message, optionally with a background image.
 * If backgroundUrl is provided, it will be shown during loading.
 *
 * @param message - Loading message to display (default: "Loading...")
 * @param backgroundUrl - Optional background image URL
 * @param containerClassName - Optional CSS class for the container
 */
export function LoadingScreen({
  message = 'Loading...',
  backgroundUrl,
  containerClassName = 'loading-screen-container'
}: LoadingScreenProps) {
  return (
    <div
      className={containerClassName}
      style={backgroundUrl ? {
        backgroundImage: `url(${backgroundUrl})`,
        backgroundSize: 'cover',
        backgroundPosition: 'center',
        backgroundRepeat: 'no-repeat'
      } : {
        backgroundColor: 'var(--forge-soot)'
      }}
    >
      <div className="loading-screen-overlay">
        <div className="loading-screen-content">
          <LoadingSpinner />
          <p className="loading-screen-message">{message}</p>
        </div>
      </div>
    </div>
  );
}
