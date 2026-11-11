import React from 'react';
import { createRoot } from 'react-dom/client';
import '../css/style.css';
import ArchivePage from './pages/ArchivePage';

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ArchivePage />
  </React.StrictMode>
);
