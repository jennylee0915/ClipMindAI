// src/App.tsx
import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './App.css';

interface ClipboardItem {
  id: string;
  content: string;
  content_type: string;
  timestamp: string;
  content_length: number;
  content_preview: string;
}

function App() {
  const [isMonitoring, setIsMonitoring] = useState(false);
  const [clipboardHistory, setClipboardHistory] = useState<ClipboardItem[]>([]);
  const [message, setMessage] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  
  const updateTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const safeInvoke = async (command: string, args?: any) => {
    try {
      console.log(`調用命令: ${command}`, args || '');
      const result = await invoke(command, args);
      console.log(`命令成功: ${command}`, result);
      return result;
    } catch (error) {
      console.error(`命令失敗: ${command}`, error);
      throw error;
    }
  };

  const fetchHistory = async () => {
    try {
      const history = await safeInvoke('get_clipboard_history') as ClipboardItem[];
      setClipboardHistory(history);
    } catch (error) {
      console.error('Failed to fetch history:', error);
    }
  };

  const startMonitoring = async () => {
    setIsLoading(true);
    try {
      const result = await safeInvoke('start_clipboard_monitoring') as string;
      setMessage(result);
      setIsMonitoring(true);
      startAutoUpdate();
    } catch (error) {
      setMessage(`Start failed: ${error}`);
      setIsMonitoring(false);
    } finally {
      setIsLoading(false);
    }
  };

  const stopMonitoring = async () => {
    setIsLoading(true);
    try {
      const result = await safeInvoke('stop_clipboard_monitoring') as string;
      setMessage(result);
      setIsMonitoring(false);
      stopAutoUpdate();
    } catch (error) {
      setMessage(`Stop failed: ${error}`);
      setIsMonitoring(false);
      stopAutoUpdate();
    } finally {
      setIsLoading(false);
    }
  };

  const startAutoUpdate = () => {
    stopAutoUpdate();
    updateTimerRef.current = setInterval(fetchHistory, 500);
  };

  const stopAutoUpdate = () => {
    if (updateTimerRef.current) {
      clearInterval(updateTimerRef.current);
      updateTimerRef.current = null;
    }
  };

  const clearHistory = async () => {
    try {
      await safeInvoke('clear_clipboard_history');
      setClipboardHistory([]);
      setMessage('Clean success');
    } catch (error) {
      setMessage(`Clean failed: ${error}`);
    }
  };

  const copyToClipboard = async (content: string) => {
    try {
      await safeInvoke('copy_item_to_clipboard', { content });
      setMessage('Copied！');
      setTimeout(() => setMessage(''), 2000);
    } catch (error) {
      setMessage(`Copy failed: ${error}`);
    }
  };

  useEffect(() => {
    fetchHistory();
    return () => stopAutoUpdate();
  }, []);

  const getTypeIconClass = (type: string) => {
    const iconClasses: Record<string, string> = {
      'Url': 'type-icon url', 
      'Email': 'type-icon email', 
      'Phone': 'type-icon phone', 
      'Financial': 'type-icon financial',
      'DateTime': 'type-icon datetime', 
      'Code': 'type-icon code', 
      'Address': 'type-icon address', 
      'PlainText': 'type-icon text'
    };
    return iconClasses[type] ;
  };

  const formatTime = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleTimeString('zh-TW', {
        hour12: false,
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit'
      });
    } catch {
      return timestamp;
    }
  };

  return (
    <div className="app-container">
      {/* 左側控制面板 */}
      <div className="sidebar">
        <div className="logo-section">
          <h1 className="app-title">
            <div className="logo-icon"></div>
            ClipMind
          </h1>
        </div>
        
        <div className="controls">
          <button 
            onClick={isMonitoring ? stopMonitoring : startMonitoring}
            className={`control-btn primary ${isMonitoring ? 'stop' : 'start'}`}
            disabled={isLoading}
          >
            {isLoading ? (
              <>
                <span className="btn-spinner"></span>
                Processing...
              </>
            ) : (
              <>
                <span className={`btn-icon ${isMonitoring ? 'stop' : 'play'}`}></span>
                {isMonitoring ? 'Stop Monitoring' : 'Start Monitoring'}
              </>
            )}
          </button>
          
          <button onClick={clearHistory} className="control-btn secondary">
            <span className="btn-icon clear"></span>
            Clean History
          </button>
          
          <button onClick={fetchHistory} className="control-btn secondary">
            <span className="btn-icon refresh"></span>
            Refresh
          </button>
        </div>

        <div className="status-section">
          <div className={`status-indicator ${isMonitoring ? 'active' : ''}`}>
            <div className="status-dot"></div>
            <span className="status-text">
              {isMonitoring ? 'Active' : 'Inactive'}
            </span>
          </div>
          
          {message && (
            <div className="status-message">{message}</div>
          )}
        </div>
      </div>

      {/* 右側歷史記錄 */}
      <div className="main-content">
        <div className="content-header">
          <h2 className="content-title">
            ClipBoard History ({clipboardHistory.length})
          </h2>
          {isMonitoring && (
            <div className="live-indicator">
              <div className="pulse-dot"></div>
              Auto updating
            </div>
          )}
        </div>
        
        <div className="history-container">
          {clipboardHistory.length === 0 ? (
            <div className="empty-placeholder">
              <div className="empty-icon"></div>
              <h3>No records yet</h3>
              <p>Copy some text after enabling monitoring</p>
            </div>
          ) : (
            <div className="history-list">
              {clipboardHistory.map((item, index) => (
                <div 
                  key={item.id} 
                  className={`history-item ${index === 0 ? 'latest' : ''}`}
                >
                  <div className="item-header">
                    <div className="item-type">
                      <span className={getTypeIconClass(item.content_type)}></span>
                      <span className="type-label">{item.content_type}</span>
                    </div>
                    <div className="item-time">
                      {formatTime(item.timestamp)}
                    </div>
                  </div>
                  
                  <div className="item-content">
                    {item.content_preview}
                  </div>
                  
                  <div className="item-footer">
                    <span className="item-size">{item.content_length} chars</span>
                    <button 
                      onClick={() => copyToClipboard(item.content)}
                      className="copy-button"
                    >
                      <span className="copy-icon"></span>
                      Copy
                    </button>
                  </div>
                  
                  {index === 0 && <div className="latest-indicator">Latest</div>}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default App;