import { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './ChatBox.css';

interface ChatMessage {
    gid: number;
    message: string;
}

interface ChatPayload {
    gid: number;
    message: string;
}

export const ChatBox = () => {
    const [messages, setMessages] = useState<ChatMessage[]>([]);
    const [inputValue, setInputValue] = useState('');
    const messagesEndRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        const unlisten = listen<ChatPayload>('chat-message-received', (event) => {
            setMessages((prev) => [...prev, event.payload]);
        });

        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    const handleKeyDown = async (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter' && inputValue.trim()) {
            try {
                await invoke('send_chat_message', { message: inputValue });
                setInputValue('');
            } catch (error) {
                console.error('Failed to send chat message:', error);
            }
        }
        // Stop propagation to prevent game inputs from triggering (like movement)
        e.stopPropagation();
    };

    return (
        <div className="chat-box">
            <div className="chat-messages">
                {messages.map((msg, index) => (
                    <div key={index} className="chat-message">
                        {msg.message}
                    </div>
                ))}
                <div ref={messagesEndRef} />
            </div>
            <div className="chat-input-container">
                <input
                    type="text"
                    className="chat-input"
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder="Press Enter to chat..."
                />
            </div>
        </div>
    );
};
