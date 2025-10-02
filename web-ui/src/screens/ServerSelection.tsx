import { invoke } from '@tauri-apps/api/core';
import { useState, useEffect } from 'react';
import { loadAsset } from '../lib/assets';
import './ServerSelection.css';

interface ServerInfo {
  ip: number;
  port: number;
  name: string;
  users: number;
  server_type: any;
  new_server: number;
}

interface ServerSelectionProps {
  servers: ServerInfo[];
  onServerSelected: () => void;
  onBackToLogin: () => void;
}

interface ServerSelectionResponse {
  success: boolean;
  error?: string;
}

export default function ServerSelection({
  servers,
  onServerSelected,
  onBackToLogin
}: ServerSelectionProps) {
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [backgroundUrl, setBackgroundUrl] = useState<string | null>(null);

  useEffect(() => {
    const loadBackground = async () => {
      try {
        const url = await loadAsset('login_screen.png');
        setBackgroundUrl(url);
      } catch (err) {
        setError('Failed to load background image');
      }
    };

    loadBackground();

    return () => {
      if (backgroundUrl) {
        URL.revokeObjectURL(backgroundUrl);
      }
    };
  }, []);

  const handleServerSelect = (index: number, server: ServerInfo) => {
    // Don't select maintenance servers
    if (server.server_type === 'Maintenance') {
      return;
    }
    setSelectedIndex(index);
  };

  const handleConnect = async () => {
    if (selectedIndex === null) {
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const result = await invoke<ServerSelectionResponse>('select_server', {
        serverIndex: selectedIndex
      });

      if (result.success) {
        onServerSelected();
      } else {
        setError(result.error || 'Server selection failed');
      }
    } catch (err) {
      setError('Network error: ' + err);
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (servers.length === 0) return;

    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (selectedIndex === null || selectedIndex === 0) {
        setSelectedIndex(servers.length - 1);
      } else {
        setSelectedIndex(selectedIndex - 1);
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (selectedIndex === null || selectedIndex === servers.length - 1) {
        setSelectedIndex(0);
      } else {
        setSelectedIndex(selectedIndex + 1);
      }
    } else if (e.key === 'Enter' && selectedIndex !== null) {
      e.preventDefault();
      handleConnect();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onBackToLogin();
    }
  };

  return (
    <div
      className="server-selection-container"
      style={backgroundUrl ? {
        backgroundImage: `url(${backgroundUrl})`,
        backgroundSize: 'cover',
        backgroundPosition: 'center',
        backgroundRepeat: 'no-repeat'
      } : {}}
      onKeyDown={handleKeyDown}
      tabIndex={0}
    >
      <div className="server-selection-box">
        <h1 className="server-selection-title">Select Server</h1>

        <div className="server-list">
          {servers.map((server, index) => (
            <div
              key={index}
              className={`server-item ${
                selectedIndex === index ? 'selected' : ''
              } ${
                server.server_type === 'Maintenance' ? 'maintenance' : ''
              }`}
              onClick={() => handleServerSelect(index, server)}
            >
              <span className="server-name">{server.name}</span>
              {server.server_type === 'Maintenance' && (
                <span className="maintenance-badge">Maintenance</span>
              )}
            </div>
          ))}
        </div>

        {error && (
          <div className="error-message">{error}</div>
        )}

        <div className="buttons-container">
          <button
            onClick={onBackToLogin}
            className="back-button"
            disabled={loading}
          >
            Back to Login
          </button>

          <button
            onClick={handleConnect}
            disabled={loading || selectedIndex === null}
            className="connect-button"
          >
            {loading ? 'Connecting...' : 'Connect'}
          </button>
        </div>
      </div>
    </div>
  );
}
