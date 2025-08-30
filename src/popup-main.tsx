// src/popup-main.tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import PopupWindow from './popup';

console.log('Popup React 應用程式啟動');

const root = ReactDOM.createRoot(document.getElementById('root') as HTMLElement);
root.render(
  <React.StrictMode>
    <PopupWindow />
  </React.StrictMode>
);

console.log('Popup React 應用程式已渲染');