import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import './Login.css';

interface ServerInfo {
  ip: number;
  port: number;
  name: string;
  users: number;
  server_type: any;
  new_server: number;
}

interface SessionData {
  username: string;
  login_id1: number;
  account_id: number;
  login_id2: number;
  sex: number;
  servers: ServerInfo[];
}

interface LoginResponse {
  success: boolean;
  error?: string;
  session_data?: SessionData;
}

interface LoginProps {
  onLoginSuccess: (servers: ServerInfo[]) => void;
}

export default function Login({ onLoginSuccess }: LoginProps) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    try {
      const result = await invoke<LoginResponse>('login', {
        request: { username, password }
      });

      if (result.success && result.session_data) {
        onLoginSuccess(result.session_data.servers);
      } else {
        setError(result.error || 'Login failed');
      }
    } catch (err) {
      setError('Network error: ' + err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="login-container">
      <div className="login-box">
        <form onSubmit={handleSubmit} className="login-form">
          <div className="input-group">
            <label htmlFor="username">Username</label>
            <input
              id="username"
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              disabled={loading}
              autoFocus
              required
            />
          </div>

          <div className="input-group">
            <label htmlFor="password">Password</label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={loading}
              required
            />
          </div>

          {error && (
            <div className="error-message">{error}</div>
          )}

          <button
            type="submit"
            disabled={loading || !username || !password}
            className="login-button"
          >
            {loading ? 'Logging in...' : 'Login'}
          </button>
        </form>
      </div>
    </div>
  );
}
