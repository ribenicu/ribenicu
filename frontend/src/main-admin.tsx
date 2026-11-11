import React from 'react';
import { createRoot } from 'react-dom/client';
import '../css/style.css';
import AdminApp from './pages/admin/AdminApp';

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AdminApp />
  </React.StrictMode>
);
