// src/popup.tsx
import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import './popup.css';

interface PopupData {
  content: string;
  contentType: string;
  actionCount?: number;
  windowSize?: {
    width: number;
    height: number;
  };
}

interface ActionSuggestion {
  id: string;
  label: string;
  hotkey: string;
  source?: 'rule' | 'ai' | 'basic';
  reason?: string;
}

interface AIResultData {
  content: string;
  actionType: string;
  processingTime?: number;
}

const PopupWindow: React.FC = () => {
  const [popupData, setPopupData] = useState<PopupData>({ content: '', contentType: '' });
  const [actions, setActions] = useState<ActionSuggestion[]>([]);
  const [userInteracted, setUserInteracted] = useState(false);
  const [loadingAI, setLoadingAI] = useState(true);
  const [aiResult, setAiResult] = useState<AIResultData | null>(null);
  const [processingAction, setProcessingAction] = useState<string | null>(null);

  const contentRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragHandleRef = useRef<HTMLDivElement>(null);

  const getTypeIconClass = (type: string) => {
    const iconClasses: Record<string, string> = {
      Url: 'type-icon url', 
      Email: 'type-icon email', 
      Phone: 'type-icon phone', 
      Financial: 'type-icon financial',
      DateTime: 'type-icon datetime', 
      Code: 'type-icon code', 
      Address: 'type-icon address', 
      PlainText: 'type-icon text'
    };
    return iconClasses[type] || 'type-icon document';
  };

  const getActionTypeInfo = (actionType: string) => {
    const actionTypes: Record<string, { iconClass: string; title: string; }> = {
      'translate': { iconClass: 'action-type-icon translate', title: 'AI Translation Result' },
      'summarize': { iconClass: 'action-type-icon summarize', title: 'AI Summary Result' },
      'summarize_webpage': { iconClass: 'action-type-icon webpage', title: 'AI Webpage Summary' },
      'explain_code': { iconClass: 'action-type-icon explain', title: 'AI Code Explanation' },
      'optimize_code': { iconClass: 'action-type-icon optimize', title: 'AI Code Optimization' },
      'add_comments': { iconClass: 'action-type-icon comment', title: 'AI Comment Generation' },
      'extract_keywords': { iconClass: 'action-type-icon keyword', title: 'AI Keyword Extraction' },
      'rewrite': { iconClass: 'action-type-icon rewrite', title: 'AI Content Rewrite' },
      'error': { iconClass: 'action-type-icon error', title: 'Processing Failed' },
    };
    return actionTypes[actionType] || { iconClass: 'action-type-icon ai', title: 'AI Processing Result' };
  };

  const getBasicActionsForType = (type: string, content: string): ActionSuggestion[] => {
    switch (type) {
      case 'Url':
        return [
          { id: 'open_browser', label: 'Open URL', hotkey: '1', source: 'basic' },
        ];

      case 'Code':
        return [
          { id: 'open_vscode', label: 'Open in VSCode', hotkey: '1', source: 'basic' },
        ];

      case 'Email':
        return [
          { id: 'compose_email', label: 'Compose Email', hotkey: '1', source: 'basic' },
        ];

      case 'Address':
        return [
          { id: 'open_maps', label: 'Open in Maps', hotkey: '1', source: 'basic' },
          { id: 'search', label: 'Search Address', hotkey: '2', source: 'basic' },
        ];

      case 'PlainText':
        return [
          { id: 'search', label: 'Google Search', hotkey: '1', source: 'basic' },
        ];

      case 'Financial':
        return [
          { id: 'search', label: 'Search Finance Info', hotkey: '1', source: 'basic' }
        ];

      default:
        return [
          { id: 'search', label: 'Search', hotkey: '1', source: 'basic' },
        ];
    }
  };

  // Get AI suggestions
  const loadActionsWithAI = async (content: string, contentType: string) => {
    const basicActions = getBasicActionsForType(contentType, content);
    setActions(basicActions);
    setLoadingAI(true);

    try {
      console.log('Getting AI suggestions...');
      const aiSuggestions = await invoke('get_ai_suggestions', {
        content,
        contentType
      }) as Array<{
        id: string;
        label: string;
        icon: string;
        hotkey: string;
        source: string;
        reason?: string;
        confidence: number;
      }>;

      console.log('AI suggestions retrieved successfully:', aiSuggestions.length, 'items');

      if (aiSuggestions.length > 0) {
        const mergedActions: ActionSuggestion[] = [];
        let hotkeyCounter = 1;

        basicActions.forEach(action => {
          mergedActions.push({
            ...action,
            hotkey: hotkeyCounter.toString()
          });
          hotkeyCounter++;
        });

        aiSuggestions.slice(0, 3).forEach(aiAction => {
          const isDuplicate = mergedActions.some(existing => 
            existing.id === aiAction.id || 
            existing.label.includes(aiAction.label)
          );

          if (!isDuplicate && hotkeyCounter <= 6) {
            mergedActions.push({
              id: aiAction.id,
              label: aiAction.label,
              hotkey: hotkeyCounter.toString(),
              source: 'ai',
              reason: aiAction.reason,
            });
            hotkeyCounter++;
          }
        });

        setActions(mergedActions);
        console.log('Action menu update completed:', mergedActions.length, 'items');
      }
    } catch (error) {
      console.warn('Failed to retrieve AI suggestions:', error);
    } finally {
      setLoadingAI(false);
    }
  };

  const executeAction = async (actionId: string) => {
    setUserInteracted(true);
    setProcessingAction(actionId);

    try {
      const isAiAction = actionId.startsWith('ai_') || 
                        actions.find(a => a.id === actionId)?.source === 'ai';

      if (isAiAction) {
        console.log('Executing AI action:', actionId);
        
        const startTime = Date.now();
        const taskType = actionId.replace('ai_', '');
        const result = await invoke('process_ai_task', {
          taskType,
          content: popupData.content,
          parameters: {}
        }) as string;
        
        const processingTime = Date.now() - startTime;
        
        setAiResult({
          content: result,
          actionType: taskType,
          processingTime,
        });
        
        console.log('AI task completed');
      } else {
        console.log('Executing basic action:', actionId);
        const result = await invoke('run_action', {
          actionId,
          content: popupData.content
        });
        
        console.log('Action executed successfully:', result);
        closePopup();
      }
    } catch (error) {
      console.error('Action execution failed:', error);
      setAiResult({
        content: `Execution failed: ${error}`,
        actionType: 'error',
      });
      setTimeout(() => {
        closePopup();
      }, 3000);
    } finally {
      setProcessingAction(null);
    }
  };

  const closePopup = async () => {
    try {
      console.log('Closing popup window');
      await invoke('close_popup');
    } catch (error) {
      console.error('Failed to close window:', error);
      try {
        const win = getCurrentWebviewWindow();
        await win.destroy();
      } catch {
        window.close();
      }
    }
  };

  const copyAiResult = async () => {
    if (aiResult?.content) {
      try {
        await invoke('copy_item_to_clipboard', { content: aiResult.content });
        console.log('AI result copied');
        
        // Provide visual feedback
        const button = document.querySelector('.result-action-btn.primary') as HTMLElement;
        if (button) {
          const originalText = button.textContent;
          button.textContent = 'Copied';
          button.style.background = 'var(--subtext)';
          setTimeout(() => {
            button.textContent = originalText;
            button.style.background = '';
          }, 1500);
        }
      } catch (error) {
        console.error('Copy failed:', error);
      }
    }
  };

  // Retry or edit function
  const retryAction = () => {
    setAiResult(null);
    // Return to action selection interface
  };

  useEffect(() => {
    const global = (window as any).clipboardData;
    if (global) {
      const contentData = {
        content: global.content || '',
        contentType: global.contentType || 'PlainText',
        actionCount: global.actionCount || 4,
        windowSize: global.windowSize
      };

      setPopupData(contentData);
      loadActionsWithAI(contentData.content, contentData.contentType);

      console.log('Popup initialization completed, content type:', contentData.contentType);
    } else {
      console.warn('No clipboard data received, using default actions');
      setActions(getBasicActionsForType('PlainText', ''));
      setLoadingAI(false);
    }

    setTimeout(() => {
      document.body.focus();
    }, 100);
  }, []);

  // HotKey function
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        closePopup();
      }

      if (/^[1-9]$/.test(e.key)) {
        const index = parseInt(e.key) - 1;
        if (index < actions.length && !aiResult) {
          executeAction(actions[index].id);
        }
      }

      if (e.ctrlKey && e.key === 'c' && aiResult) {
        e.preventDefault();
        copyAiResult();
      }

      if (e.ctrlKey && e.key === 'r' && aiResult) {
        e.preventDefault();
        retryAction();
      }
    };

    window.addEventListener('keydown', handleKeyDown, { capture: true });
    return () => {
      window.removeEventListener('keydown', handleKeyDown, { capture: true });
    };
  }, [actions, aiResult]);

  useEffect(() => {
    if (!userInteracted && !aiResult) {
      const timer = setTimeout(() => {
        if (!userInteracted) {
          closePopup();
        }
      }, 30000);
      return () => clearTimeout(timer);
    }

    if (aiResult && !aiResult.actionType.includes('error')) {
      const timer = setTimeout(() => {
        closePopup();
      }, 15000);
      return () => clearTimeout(timer);
    }
  }, [userInteracted, aiResult]);

  const handleClick = () => {
    setUserInteracted(true);
  };

  if (aiResult) {
    const actionInfo = getActionTypeInfo(aiResult.actionType);
    
    return (
      <div className="popup-container" onClick={handleClick}>
        <div className="popup-content ai-result-mode">
          {/* Drag handle */}
          <div className="drag-handle" data-tauri-drag-region>
            <div className="drag-indicator"></div>
          </div>

          <div className="popup-header">
            <div className="content-type ai-result-header">
              <span className={actionInfo.iconClass}></span>
              <div className="ai-result-info">
                <span className="type-text">{actionInfo.title}</span>
              </div>
              {aiResult.processingTime && (
                <span className="processing-time">{aiResult.processingTime}ms</span>
              )}
            </div>
            <button className="close-btn" onClick={closePopup}></button>
          </div>
          
          <div className="ai-result-content">
            <div className="result-text-container">
              <div 
                className={`result-text ${aiResult.actionType === 'error' ? 'error' : ''}`} 
                title="Click to copy"
                onClick={copyAiResult}
              >
                {aiResult.content}
              </div>
            </div>
            
            <div className="result-actions">
              <button 
                className="result-action-btn secondary"
                onClick={retryAction}
                title="Return to reselect (Ctrl+R)"
              >
                <span className="result-action-icon retry"></span>
                Retry
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="popup-container"
      onClick={handleClick}
      tabIndex={0}
      style={{ outline: 'none' }}
    >
      <div className="popup-content">
        {/* Drag handle */}
        <div className="drag-handle" data-tauri-drag-region ref={dragHandleRef}>
          <div className="drag-indicator"></div>
        </div>

        <div className="popup-header">
          <div className="content-type">
            <span className={getTypeIconClass(popupData.contentType)}></span>
            <span className="type-text">{popupData.contentType}</span>
            {loadingAI && (
              <span className="ai-loading">
                <span className="ai-loading-icon"></span>
              </span>
            )}
          </div>
          <button className="close-btn" onClick={closePopup}></button>
        </div>
        
        <div ref={contentRef} className="copied-content">
          {popupData.content || 'Waiting for clipboard content...'}
        </div>
        
        <div className="suggestions-title">
          Recommended Actions {loadingAI && <span className="loading-text">(Getting AI suggestions...)</span>}
        </div>
        
        <div className="action-buttons">
          {actions.map((action) => (
            <button
              key={action.id}
              className={`action-btn ${action.source === 'ai' ? 'ai-enhanced' : ''}`}
              onClick={() => executeAction(action.id)}
              disabled={processingAction === action.id}
              title={action.reason}
            >
              <div className="action-left">
                <span className="action-icon" data-action={action.id}></span>
                <span className="action-label">{action.label}</span>
              </div>
              
              <div className="action-right">
                {processingAction === action.id ? (
                  <div className="loading-spinner"></div>
                ) : (
                  <kbd className="hotkey">{action.hotkey}</kbd>
                )}
              </div>
            </button>
          ))}
        </div>
        
        <div className="popup-footer">
          <span>Press 1-{actions.length} to perform actions • ESC Close</span>
        </div>
      </div>
    </div>
  );
};

export default PopupWindow;